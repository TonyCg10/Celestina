#include "shellservice.h"

#include <QDBusConnection>
#include <QDBusError>
#include <QDebug>
#include <QVariantList>

#include "niriclient.h"
#include "overlaycontroller.h"

namespace {
// Bumped only when the payloads change shape. Consumers read it before they
// read anything else; new keys never need a bump.
constexpr int shellStateVersion = 1;
// State changes are worth publishing, not worth publishing at snapshot rate.
constexpr int stateCoalesceMs = 100;
constexpr int maxWorkspaceIndex = 255;
constexpr qsizetype maxOutputNameLength = 128;
} // namespace

ShellService::ShellService(NiriClient *niri, QObject *parent)
    : QObject(parent)
    , m_niri(niri)
{
    m_stateTimer.setSingleShot(true);
    m_stateTimer.setInterval(stateCoalesceMs);
    connect(&m_stateTimer, &QTimer::timeout, this, &ShellService::publishState);

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

qulonglong ShellService::Command(const QString &verb, const QVariantMap &options)
{
    if (verb == QStringLiteral("focus-workspace"))
        return focusWorkspace(options);
    if (verb == QStringLiteral("launcher-toggle"))
        return toggleOverlay(m_launcher, verb);
    if (verb == QStringLiteral("clipboard-toggle"))
        return toggleOverlay(m_clipboard, verb);

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
