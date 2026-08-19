#include "shellservice.h"

#include <QDBusConnection>
#include <QDBusError>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>
#include <QDebug>
#include <QVariantList>

#include <optional>
#include <utility>

#include <QMetaType>
#include <QRectF>
#include "niriclient.h"
#include "lockcontroller.h"
#include "bubbleanchorsource.h"
#include "overlaycontroller.h"
#include "shellprovidersclient.h"

namespace {
// Bumped only when the payloads change shape. Consumers read it before they
// read anything else; new keys never need a bump.
constexpr int shellStateVersion = 1;
// State changes are worth publishing, not worth publishing at snapshot rate.
constexpr int stateCoalesceMs = 100;
constexpr int maxWorkspaceIndex = 255;
constexpr qsizetype maxOutputNameLength = 128;
// A verb arrives from the bus and any client may send any string. Quoting one
// back in a refusal is useful; quoting back a megabyte of it is a way to make
// the shell carry a caller's payload, so what is echoed is cut first.
constexpr qsizetype maxEchoedVerbLength = 64;
// How often the shell looks for a session request that has run out of time.
constexpr int sessionSweepMs = 250;
// wpctl answers immediately; a monitor over DDC takes seconds, and refusing it
// early would report a failure the session is about to disprove.
constexpr qint64 audioTimeoutMs = 3000;
constexpr qint64 brightnessTimeoutMs = 30000;
// Taking or releasing a hold is starting or killing a process; the provider
// republishes as soon as it has.
constexpr qint64 holdTimeoutMs = 3000;

// The session manager every systemd session already has. The shell asks it and
// reimplements nothing: what "power off" means for this machine — inhibitors,
// other sessions, unsaved work — is logind's to know.
constexpr auto logindService = "org.freedesktop.login1";
constexpr auto logindPath = "/org/freedesktop/login1";
constexpr auto logindInterface = "org.freedesktop.login1.Manager";

// The form of a verb that is safe to repeat back to whoever sent it.
QString echoed(const QString &verb)
{
    if (verb.size() <= maxEchoedVerbLength)
        return verb;
    return verb.left(maxEchoedVerbLength) + QStringLiteral("...");
}

// Which provider carries a session verb, and what would count as having seen
// it happen.
//
// The verb vocabulary itself belongs to `celestina-shell-core` and is checked
// where the request is carried out: this table decides only routing and
// observability, so an unknown verb reaches the helper and comes back with the
// helper's own refusal rather than a second, drifting rulebook here.
std::optional<SessionRequests::Expectation> sessionExpectation(
    const QString &verb,
    const QVariantMap &options
)
{
    const auto audio = [](const QString &field, const QVariant &value) {
        return SessionRequests::Expectation {
            QStringLiteral("audio"), field, value, audioTimeoutMs};
    };

    if (verb == QStringLiteral("volume-set")) {
        return audio(
            QStringLiteral("volume"),
            options.value(QStringLiteral("level"))
        );
    }
    if (verb == QStringLiteral("mute-on") || verb == QStringLiteral("mute-off"))
        return audio(QStringLiteral("muted"), verb.endsWith(QStringLiteral("-on")));
    if (verb == QStringLiteral("mic-mute-on")
        || verb == QStringLiteral("mic-mute-off")) {
        return audio(
            QStringLiteral("micMuted"),
            verb.endsWith(QStringLiteral("-on"))
        );
    }
    // A step and a toggle have no target the shell can name in advance: what
    // confirms them is the provider reporting something other than what the
    // panel was already showing.
    if (verb.startsWith(QStringLiteral("volume-"))
        || verb.startsWith(QStringLiteral("mute-"))
        || verb.startsWith(QStringLiteral("mic-mute-"))) {
        return audio(QString(), QVariant());
    }

    // Night light and staying awake are held states: `on` and `off` name what
    // the provider must end up reporting, while a toggle is confirmed by that
    // provider reporting anything else than it was.
    for (const QString &held : {QStringLiteral("night-light"), QStringLiteral("caffeine")}) {
        if (!verb.startsWith(held + u'-'))
            continue;

        const bool absolute = verb.endsWith(QStringLiteral("-on"))
            || verb.endsWith(QStringLiteral("-off"));
        return SessionRequests::Expectation {
            held,
            absolute ? QStringLiteral("active") : QString(),
            absolute ? QVariant(verb.endsWith(QStringLiteral("-on"))) : QVariant(),
            holdTimeoutMs,
        };
    }

    if (verb.startsWith(QStringLiteral("brightness-"))) {
        // Brightness is per monitor, so the field being watched is the output
        // the request names.
        const QString output =
            verb == QStringLiteral("brightness-set")
                ? options.value(QStringLiteral("output")).toString()
                : QString();
        return SessionRequests::Expectation {
            QStringLiteral("brightness"),
            output,
            options.value(QStringLiteral("level")),
            brightnessTimeoutMs,
        };
    }

    return std::nullopt;
}
} // namespace

ShellService::ShellService(NiriClient *niri, QObject *parent)
    : QObject(parent)
{
    m_stateTimer.setSingleShot(true);
    m_stateTimer.setInterval(stateCoalesceMs);
    connect(&m_stateTimer, &QTimer::timeout, this, &ShellService::publishState);

    m_clock.start();
    m_sessionTimer.setInterval(sessionSweepMs);
    connect(&m_sessionTimer, &QTimer::timeout, this, [this] {
        m_sessionRequests.expire(m_clock.elapsed());
        reportSessionOutcomes();
    });

    setNiriClient(niri);
}

void ShellService::setNiriClient(NiriClient *niri)
{
    if (m_niri == niri)
        return;

    if (m_niri)
        disconnect(m_niri.data(), nullptr, this, nullptr);

    m_niri = niri;
    if (!m_niri)
        return;

    connect(m_niri, &NiriClient::changed, this, [this] {
        if (!m_stateTimer.isActive())
            m_stateTimer.start();
    });
    connect(
        m_niri,
        &NiriClient::focusRequestChanged,
        this,
        &ShellService::reportFocusRequest
    );
    connect(m_niri, &NiriClient::actionFinished, this, &ShellService::reportAction);
}

int ShellService::maxRequestLifetimeMs()
{
    return static_cast<int>(brightnessTimeoutMs) + sessionSweepMs;
}

QString ShellService::serviceName()
{
    return QStringLiteral("org.celestina.Shell");
}

QString ShellService::objectPath()
{
    return QStringLiteral("/org/celestina/Shell1");
}

QString ShellService::interfaceName()
{
    return QStringLiteral("org.celestina.Shell1");
}

ShellService::Attachment ShellService::attach(const QDBusConnection &bus)
{
    if (!bus.isConnected()) {
        qWarning().noquote()
            << "Celestina found no session bus; the shell command channel is "
               "unavailable:"
            << bus.lastError().message();
        return Attachment::NoBus;
    }

    QDBusConnection connection = bus;
    if (!connection.registerObject(
            objectPath(),
            this,
            QDBusConnection::ExportAllSlots | QDBusConnection::ExportAllSignals
        )) {
        qWarning().noquote()
            << "Celestina could not export its shell object:"
            << connection.lastError().message();
        return Attachment::NoBus;
    }

    if (!connection.registerService(serviceName())) {
        connection.unregisterObject(objectPath());
        return Attachment::NameTaken;
    }

    return Attachment::Owned;
}

qulonglong ShellService::refuse(QDBusError::ErrorType type, const QString &message)
{
    m_refusalReason = message;
    // Only a real dispatch has a message to answer. Without one there is no
    // call context behind `QDBusContext`, and replying would dereference it.
    if (calledFromDBus())
        sendErrorReply(type, message);
    else
        qWarning().noquote() << "Celestina refused a shell request:" << message;
    return 0;
}

QString ShellService::takeRefusalReason()
{
    return std::exchange(m_refusalReason, QString());
}

QVariantMap ShellService::GetState()
{
    QVariantMap state;
    state.insert(QStringLiteral("version"), shellStateVersion);
    state.insert(
        QStringLiteral("niriAvailable"),
        m_niri ? m_niri->available() : false
    );
    state.insert(
        QStringLiteral("workspaces"),
        m_niri ? m_niri->workspaces() : QVariantList()
    );
    return state;
}

void ShellService::publishState()
{
    emit Changed(GetState());
}

void ShellService::setLauncherController(OverlayController *controller)
{
    m_launcher = controller;
}

void ShellService::setClipboardController(OverlayController *controller)
{
    m_clipboard = controller;
}

void ShellService::setNotificationCentreController(OverlayController *controller)
{
    m_notificationCentre = controller;
}

void ShellService::setControlCentreController(OverlayController *controller)
{
    m_controlCentre = controller;
}

void ShellService::setBubbleSelectorController(OverlayController *controller)
{
    m_bubbleSelector = controller;
}

void ShellService::setSessionMenuController(OverlayController *controller)
{
    m_sessionMenu = controller;
}

void ShellService::setBubbleAnchorSource(BubbleAnchorSource *anchors)
{
    m_anchors = anchors;
}

// The presentation hint for a minimize triggered with no surface of its own.
//
// Reduced motion wins over the anchor: someone who asked for less movement is asking for no
// travel at all, not for travel somewhere else. An absent panel, an unknown output, or an
// unusable rectangle simply sends nothing, which asks Niri for its ordinary motion; a hint is
// never allowed to hold up or refuse the state change it decorates.
QVariantMap ShellService::minimizeTransition() const
{
    if (!m_anchors || !m_niri)
        return {};
    if (m_anchors->reducedMotion())
        return {{QStringLiteral("transition"), QStringLiteral("disabled")}};
    // The focused window is the one Niri will minimize, so its monitor is the one whose
    // bubbles it should travel to.
    const QString output = m_niri->focusedOutput();
    const QRectF anchor = m_anchors->bubbleAnchorFor(output);
    if (output.isEmpty() || !anchor.isValid() || anchor.width() <= 0 || anchor.height() <= 0)
        return {};
    return {
        {QStringLiteral("transition"), QStringLiteral("anchored")},
        {QStringLiteral("anchor_output"), output},
        {QStringLiteral("anchor_x"), anchor.x()},
        {QStringLiteral("anchor_y"), anchor.y()},
        {QStringLiteral("anchor_width"), anchor.width()},
        {QStringLiteral("anchor_height"), anchor.height()},
    };
}

qulonglong ShellService::minimizeWindow(const QVariantMap &options)
{
    // The shell's own frame can still name Melibea while its projection has been withdrawn.
    // The boolean is checked as a boolean: a truthy string would say available for a
    // provider that had published nothing, and this verb would then report a request that
    // nothing was ever going to answer.
    const QVariant available =
        m_providers
        ? m_providers->providers().value(QStringLiteral("melibea")).toMap().value(
              QStringLiteral("available"))
        : QVariant();
    if (available.metaType().id() != QMetaType::Bool || !available.toBool()) {
        return refuse(
            QDBusError::Failed,
            QStringLiteral("this shell has no authoritative Melibea to minimize through")
        );
    }

    QVariantMap request = minimizeTransition();
    // Naming no window asks Niri to resolve whichever one is focused. Anything the caller
    // does name must be an exact id, never a rounded or reinterpreted one.
    const QVariant target = options.value(QStringLiteral("window_id"));
    if (target.isValid() && !target.isNull()) {
        const QString exact = target.toString();
        bool parsed = false;
        exact.toULongLong(&parsed);
        if (!parsed) {
            return refuse(
                QDBusError::InvalidArgs,
                QStringLiteral("'minimize' needs an exact window id or none at all")
            );
        }
        request.insert(QStringLiteral("window_id"), exact);
    }

    const qulonglong helperRequestId = m_providers->sendCommand(
        QStringLiteral("melibea"), QStringLiteral("minimize"), request);
    if (helperRequestId == 0) {
        return refuse(
            QDBusError::Failed,
            QStringLiteral("this shell could not send the minimize to Melibea")
        );
    }

    // The helper numbers its own requests, so the caller is answered under a shell id and the
    // two are matched up when the terminal result arrives.
    const qulonglong requestId = ++m_lastRequestId;
    m_providerRequests.insert(helperRequestId, requestId);
    m_providerVerbs.insert(helperRequestId, QStringLiteral("minimize"));
    return requestId;
}

void ShellService::setProvidersClient(ShellProvidersClient *providers)
{
    m_providers = providers;
    if (!m_providers)
        return;

    connect(
        m_providers,
        &ShellProvidersClient::commandResult,
        this,
        [this](qulonglong helperRequestId, const QString &state, const QString &reason) {
            m_sessionRequests.acknowledge(
                helperRequestId,
                state,
                reason,
                m_clock.elapsed()
            );
            reportSessionOutcomes();
            reportProviderAction(helperRequestId, state, reason);
        }
    );
    connect(m_providers, &ShellProvidersClient::changed, this, [this] {
        // A helper that went away can no longer report anything happening, so
        // what is in flight fails now instead of waiting out its timeout.
        if (!m_providers->available()) {
            m_sessionRequests.failAll(
                QStringLiteral("the provider helper is unavailable")
            );
        } else {
            m_sessionRequests.applyProviders(
                m_providers->providers(),
                m_clock.elapsed()
            );
        }
        reportSessionOutcomes();
    });
}

qulonglong ShellService::Command(const QString &verb, const QVariantMap &options)
{
    if (verb == QStringLiteral("focus-workspace"))
        return focusWorkspace(options);
    if (verb == QStringLiteral("launcher-toggle"))
        return toggleOverlay(m_launcher, verb);
    if (verb == QStringLiteral("clipboard-toggle"))
        return toggleOverlay(m_clipboard, verb);
    if (verb == QStringLiteral("notifications-toggle"))
        return toggleOverlay(m_notificationCentre, verb);
    if (verb == QStringLiteral("control-centre-toggle"))
        return toggleOverlay(m_controlCentre, verb);
    if (verb == QStringLiteral("bubbles-toggle"))
        return toggleBubbleSelector(verb);
    if (verb == QStringLiteral("minimize"))
        return minimizeWindow(options);
    if (verb == QStringLiteral("session-menu-toggle"))
        return toggleOverlay(m_sessionMenu, verb);
    if (const auto expectation = sessionExpectation(verb, options))
        return requestSession(verb, options, *expectation);
    if (verb == QStringLiteral("displays-off"))
        return powerOffMonitors();
    if (verb == QStringLiteral("log-out"))
        return logOut();
    if (verb == QStringLiteral("reboot"))
        return askLogind(verb, QStringLiteral("Reboot"));
    if (verb == QStringLiteral("power-off"))
        return askLogind(verb, QStringLiteral("PowerOff"));
    // `suspend` is deliberately the same path as `lock-and-suspend`: a
    // session that suspends unlocked wakes up unlocked, so there is no verb
    // here that sleeps an uncovered screen.
    if (verb == QStringLiteral("suspend")
        || verb == QStringLiteral("lock")
        || verb == QStringLiteral("lock-and-suspend")) {
        return lockSession(verb);
    }

    return refuse(
        QDBusError::UnknownMethod,
        QStringLiteral("this shell does not serve the verb '%1'").arg(echoed(verb))
    );
}

// The lock verbs. Every one of them is fail-closed: with no lock provider, or
// a lock that will not start, this refuses rather than reporting a success
// that would leave the session open — and `suspend` refuses rather than
// sleeping an uncovered screen.
qulonglong ShellService::lockSession(const QString &verb)
{
    if (!m_lock) {
        return refuse(
            QDBusError::NotSupported,
            QStringLiteral(
                "this shell has no provider for a session locker, so '%1' is "
                "refused rather than half-performed"
            ).arg(echoed(verb))
        );
    }

    if (verb == QStringLiteral("lock")) {
        if (!m_lock->lock()) {
            return refuse(
                QDBusError::Failed,
                QStringLiteral("the session lock could not be started")
            );
        }
        const qulonglong requestId = ++m_lastRequestId;
        // Started, not covered: the compositor confirms the cover later, and
        // `lock` deliberately does not wait for it — a person who asked to
        // lock is not waiting on a reply.
        reportOutcome(requestId, verb, QStringLiteral("confirmed"), QString());
        return requestId;
    }

    // Suspending: the answer comes when the lock is confirmed and logind has
    // replied, never before. A refusal here means the session is still awake
    // and still uncovered, which is the visible, recoverable failure.
    const qulonglong requestId = ++m_lastRequestId;
    m_lock->lockAndSuspend([this, requestId, verb](const QString &failure) {
        if (failure.isEmpty()) {
            reportOutcome(requestId, verb, QStringLiteral("confirmed"),
                          QString());
            return;
        }
        reportOutcome(requestId, verb, QStringLiteral("failed"), failure);
    });
    return requestId;
}

void ShellService::setLockController(LockController *controller)
{
    m_lock = controller;
}

qulonglong ShellService::toggleBubbleSelector(const QString &verb)
{
    // The selector must belong to a monitor even when nothing pointed at one, because that is
    // what carries the bubble anchor its restores travel to. The focused output is the one
    // whose bubbles the person is looking at.
    if (!m_bubbleSelector) {
        return refuse(
            QDBusError::Failed,
            QStringLiteral("this shell has no '%1' surface").arg(echoed(verb))
        );
    }
    const QString output = (m_anchors && m_niri) ? m_niri->focusedOutput() : QString();
    const QRectF anchor = m_anchors ? m_anchors->bubbleAnchorFor(output) : QRectF();
    m_bubbleSelector->toggleWithBubbleAnchor(output, anchor);

    const qulonglong requestId = ++m_lastRequestId;
    QVariantMap details;
    details.insert(QStringLiteral("version"), shellStateVersion);
    details.insert(QStringLiteral("verb"), verb);
    emit CommandResult(requestId, QStringLiteral("confirmed"), details);
    return requestId;
}

qulonglong ShellService::toggleOverlay(OverlayController *controller, const QString &verb)
{
    if (!controller) {
        return refuse(
            QDBusError::Failed,
            QStringLiteral("this shell has no '%1' surface").arg(echoed(verb))
        );
    }

    controller->toggle();

    const qulonglong requestId = ++m_lastRequestId;
    QVariantMap details;
    details.insert(QStringLiteral("version"), shellStateVersion);
    details.insert(QStringLiteral("verb"), verb);
    // A toggle either opened or closed the overlay before this line runs —
    // there is nothing pending to report later, unlike a compositor request.
    emit CommandResult(requestId, QStringLiteral("confirmed"), details);
    return requestId;
}

qulonglong ShellService::powerOffMonitors()
{
    // The compositor owns display power; the shell asks and reimplements
    // nothing. There is also no "on": any input wakes the outputs.
    const qulonglong niriRequestId = m_niri ? m_niri->requestDisplaysOff() : 0;
    if (niriRequestId == 0) {
        return refuse(
            QDBusError::Failed,
            QStringLiteral(
                "the shell could not ask the compositor to blank the outputs"
            )
        );
    }

    const qulonglong requestId = ++m_lastRequestId;
    m_actionRequests.insert(niriRequestId, requestId);
    m_actionVerbs.insert(niriRequestId, QStringLiteral("displays-off"));
    return requestId;
}

void ShellService::reportOutcome(
    qulonglong requestId,
    const QString &verb,
    const QString &state,
    const QString &reason
)
{
    QVariantMap details;
    details.insert(QStringLiteral("version"), shellStateVersion);
    details.insert(QStringLiteral("verb"), verb);
    if (!reason.isEmpty())
        details.insert(QStringLiteral("reason"), reason);
    emit CommandResult(requestId, state, details);
}

qulonglong ShellService::logOut()
{
    // The compositor owns the session; the shell asks it to end and reports
    // only whether the request could be made.
    const qulonglong niriRequestId = m_niri ? m_niri->requestLogOut() : 0;
    if (niriRequestId == 0) {
        return refuse(
            QDBusError::Failed,
            QStringLiteral("the shell could not ask the compositor to end the session")
        );
    }

    const qulonglong requestId = ++m_lastRequestId;
    m_actionRequests.insert(niriRequestId, requestId);
    m_actionVerbs.insert(niriRequestId, QStringLiteral("log-out"));
    return requestId;
}

qulonglong ShellService::askLogind(const QString &verb, const QString &method)
{
    QDBusConnection bus = QDBusConnection::systemBus();
    if (!bus.isConnected()) {
        return refuse(
            QDBusError::Failed,
            QStringLiteral("the shell cannot reach the session manager")
        );
    }

    QDBusMessage call = QDBusMessage::createMethodCall(
        QString::fromLatin1(logindService),
        QString::fromLatin1(logindPath),
        QString::fromLatin1(logindInterface),
        method
    );
    // `false` is "do not ask me to authenticate interactively": a shell cannot
    // answer a polkit prompt, so a session that is not allowed to do this must
    // fail visibly instead of hanging on a dialogue nobody will see.
    call.setArguments({false});

    const qulonglong requestId = ++m_lastRequestId;
    auto *watcher = new QDBusPendingCallWatcher(bus.asyncCall(call), this);
    connect(
        watcher,
        &QDBusPendingCallWatcher::finished,
        this,
        [this, requestId, verb](QDBusPendingCallWatcher *finished) {
            const QDBusPendingReply<> reply = *finished;
            finished->deleteLater();
            // logind answering is the outcome. Nothing here assumes the machine
            // is going down: if it refuses, the session stays exactly as it is.
            if (reply.isError()) {
                reportOutcome(
                    requestId,
                    verb,
                    QStringLiteral("failed"),
                    reply.error().message()
                );
                return;
            }
            reportOutcome(requestId, verb, QStringLiteral("confirmed"), QString());
        }
    );
    return requestId;
}

qulonglong ShellService::requestSession(
    const QString &verb,
    const QVariantMap &options,
    const SessionRequests::Expectation &expectation
)
{
    if (!m_providers) {
        return refuse(
            QDBusError::Failed,
            QStringLiteral(
                "this shell has no provider helper, so it cannot serve '%1'"
            ).arg(echoed(verb))
        );
    }
    if (m_sessionRequests.isFull()) {
        // Sending a request the shell cannot track would leave the caller
        // waiting on a result that never comes.
        return refuse(
            QDBusError::Failed,
            QStringLiteral("this shell is already waiting on too many requests")
        );
    }

    // Captured before the request is sent, so the value a step is compared
    // against is the one the panel was showing when it was asked for.
    const QVariantMap before = m_providers->providers();
    const qulonglong helperRequestId =
        m_providers->sendCommand(expectation.provider, verb, options);
    if (helperRequestId == 0) {
        return refuse(
            QDBusError::Failed,
            QStringLiteral(
                "the shell could not reach the provider that serves '%1'"
            ).arg(echoed(verb))
        );
    }

    const qulonglong requestId = ++m_lastRequestId;
    m_sessionRequests.begin(
        requestId,
        helperRequestId,
        verb,
        expectation,
        before,
        m_clock.elapsed()
    );
    m_sessionTimer.start();
    reportSessionOutcomes();
    return requestId;
}

void ShellService::reportSessionOutcomes()
{
    const QList<SessionRequests::Outcome> outcomes =
        m_sessionRequests.takeOutcomes();
    for (const SessionRequests::Outcome &outcome : outcomes) {
        QVariantMap details;
        details.insert(QStringLiteral("version"), shellStateVersion);
        details.insert(QStringLiteral("verb"), outcome.verb);
        if (!outcome.reason.isEmpty())
            details.insert(QStringLiteral("reason"), outcome.reason);
        emit CommandResult(outcome.requestId, outcome.state, details);
    }

    if (m_sessionRequests.isEmpty())
        m_sessionTimer.stop();
}

// A provider-backed verb answering under its own request id.
//
// `accepted` only says the helper ran a tool; the caller is waiting to hear that the machine
// changed, so nothing is reported until a terminal state arrives. Reporting acceptance here
// would tell `celestina msg` a minimize had succeeded before Niri had done it.
void ShellService::reportProviderAction(
    qulonglong helperRequestId,
    const QString &state,
    const QString &reason
)
{
    if (state == QStringLiteral("accepted"))
        return;

    const auto request = m_providerRequests.constFind(helperRequestId);
    if (request == m_providerRequests.constEnd())
        return;

    reportOutcome(request.value(), m_providerVerbs.value(helperRequestId), state, reason);
    m_providerRequests.remove(helperRequestId);
    m_providerVerbs.remove(helperRequestId);
}

void ShellService::reportAction(
    qulonglong niriRequestId,
    const QString &state,
    const QString &reason
)
{
    const auto request = m_actionRequests.constFind(niriRequestId);
    if (request == m_actionRequests.constEnd())
        return;

    reportOutcome(request.value(), m_actionVerbs.value(niriRequestId), state, reason);
    m_actionRequests.remove(niriRequestId);
    m_actionVerbs.remove(niriRequestId);
}

qulonglong ShellService::focusWorkspace(const QVariantMap &options)
{
    const QString output = options.value(QStringLiteral("output")).toString();
    bool numeric = false;
    const int index =
        options.value(QStringLiteral("index")).toInt(&numeric);
    if (output.isEmpty() || output.size() > maxOutputNameLength || !numeric
        || index < 1 || index > maxWorkspaceIndex) {
        return refuse(
            QDBusError::InvalidArgs,
            QStringLiteral(
                "focus-workspace needs output=<name> and index=<1..255>"
            )
        );
    }

    const qulonglong niriRequestId =
        m_niri ? m_niri->requestWorkspaceFocus(output, index) : 0;
    if (niriRequestId == 0) {
        // The compositor adapter refused or could not carry the request. A
        // request that was never sent is an error, not a pending id.
        return refuse(
            QDBusError::Failed,
            QStringLiteral(
                "the shell could not request that workspace; it may not exist "
                "on that output, or the compositor adapter is unavailable"
            )
        );
    }

    const qulonglong requestId = ++m_lastRequestId;
    m_focusRequests.insert(niriRequestId, requestId);
    return requestId;
}

void ShellService::reportFocusRequest(
    qulonglong niriRequestId,
    const QString &state
)
{
    const auto request = m_focusRequests.constFind(niriRequestId);
    if (request == m_focusRequests.constEnd())
        return;

    const qulonglong requestId = request.value();
    QVariantMap details;
    details.insert(QStringLiteral("version"), shellStateVersion);
    details.insert(QStringLiteral("verb"), QStringLiteral("focus-workspace"));
    emit CommandResult(requestId, state, details);

    // "pending" is the only state a request leaves behind; the rest end it.
    if (state != QStringLiteral("pending"))
        m_focusRequests.remove(niriRequestId);
}
