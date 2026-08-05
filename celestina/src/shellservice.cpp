#include "shellservice.h"

#include <QDBusConnection>
#include <QDBusError>
#include <QDebug>
#include <QVariantList>

#include <optional>

#include "niriclient.h"
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
// How often the shell looks for a session request that has run out of time.
constexpr int sessionSweepMs = 250;
// wpctl answers immediately; a monitor over DDC takes seconds, and refusing it
// early would report a failure the session is about to disprove.
constexpr qint64 audioTimeoutMs = 3000;
constexpr qint64 brightnessTimeoutMs = 30000;
// Taking or releasing a hold is starting or killing a process; the provider
// republishes as soon as it has.
constexpr qint64 holdTimeoutMs = 3000;

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
    , m_niri(niri)
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
    if (const auto expectation = sessionExpectation(verb, options))
        return requestSession(verb, options, *expectation);
    if (verb == QStringLiteral("displays-off"))
        return powerOffMonitors();
    if (verb == QStringLiteral("lock") || verb == QStringLiteral("lock-and-suspend")) {
        // Fail-closed by contract: this shell has no locker provider, and a
        // shell that cannot lock says so instead of reporting success and
        // leaving the session open. The refusal is the seam — a provider is
        // wired in here, and until one is, nothing here pretends.
        sendErrorReply(
            QDBusError::NotSupported,
            QStringLiteral(
                "this shell has no provider for a session locker, so '%1' is "
                "refused rather than half-performed"
            ).arg(verb)
        );
        return 0;
    }

    sendErrorReply(
        QDBusError::UnknownMethod,
        QStringLiteral("this shell does not serve the verb '%1'").arg(verb)
    );
    return 0;
}

qulonglong ShellService::toggleOverlay(OverlayController *controller, const QString &verb)
{
    if (!controller) {
        sendErrorReply(
            QDBusError::Failed,
            QStringLiteral("this shell has no '%1' surface").arg(verb)
        );
        return 0;
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
        sendErrorReply(
            QDBusError::Failed,
            QStringLiteral(
                "the shell could not ask the compositor to blank the outputs"
            )
        );
        return 0;
    }

    const qulonglong requestId = ++m_lastRequestId;
    m_actionRequests.insert(niriRequestId, requestId);
    return requestId;
}

qulonglong ShellService::requestSession(
    const QString &verb,
    const QVariantMap &options,
    const SessionRequests::Expectation &expectation
)
{
    if (!m_providers) {
        sendErrorReply(
            QDBusError::Failed,
            QStringLiteral(
                "this shell has no provider helper, so it cannot serve '%1'"
            ).arg(verb)
        );
        return 0;
    }
    if (m_sessionRequests.isFull()) {
        // Sending a request the shell cannot track would leave the caller
        // waiting on a result that never comes.
        sendErrorReply(
            QDBusError::Failed,
            QStringLiteral("this shell is already waiting on too many requests")
        );
        return 0;
    }

    // Captured before the request is sent, so the value a step is compared
    // against is the one the panel was showing when it was asked for.
    const QVariantMap before = m_providers->providers();
    const qulonglong helperRequestId =
        m_providers->sendCommand(expectation.provider, verb, options);
    if (helperRequestId == 0) {
        sendErrorReply(
            QDBusError::Failed,
            QStringLiteral(
                "the shell could not reach the provider that serves '%1'"
            ).arg(verb)
        );
        return 0;
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

void ShellService::reportAction(
    qulonglong niriRequestId,
    const QString &state,
    const QString &reason
)
{
    const auto request = m_actionRequests.constFind(niriRequestId);
    if (request == m_actionRequests.constEnd())
        return;

    QVariantMap details;
    details.insert(QStringLiteral("version"), shellStateVersion);
    details.insert(QStringLiteral("verb"), QStringLiteral("displays-off"));
    if (!reason.isEmpty())
        details.insert(QStringLiteral("reason"), reason);
    emit CommandResult(request.value(), state, details);
    m_actionRequests.remove(niriRequestId);
}

qulonglong ShellService::focusWorkspace(const QVariantMap &options)
{
    const QString output = options.value(QStringLiteral("output")).toString();
    bool numeric = false;
    const int index =
        options.value(QStringLiteral("index")).toInt(&numeric);
    if (output.isEmpty() || output.size() > maxOutputNameLength || !numeric
        || index < 1 || index > maxWorkspaceIndex) {
        sendErrorReply(
            QDBusError::InvalidArgs,
            QStringLiteral(
                "focus-workspace needs output=<name> and index=<1..255>"
            )
        );
        return 0;
    }

    const qulonglong niriRequestId =
        m_niri ? m_niri->requestWorkspaceFocus(output, index) : 0;
    if (niriRequestId == 0) {
        // The compositor adapter refused or could not carry the request. A
        // request that was never sent is an error, not a pending id.
        sendErrorReply(
            QDBusError::Failed,
            QStringLiteral(
                "the shell could not request that workspace; it may not exist "
                "on that output, or the compositor adapter is unavailable"
            )
        );
        return 0;
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
