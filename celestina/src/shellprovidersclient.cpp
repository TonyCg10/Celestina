#include "shellprovidersclient.h"

#include <QDebug>
#include <QGuiApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTimer>
#include <QWindow>

namespace {
constexpr qint64 readChunkBytes = 64 * 1024;
constexpr int initialRestartDelayMs = 250;
constexpr int maximumRestartDelayMs = 10 * 1000;
// A bounded provider command may still be finishing when stdin closes. Give
// the helper enough time to drain that command and release every held child
// before SIGTERM becomes necessary.
constexpr int gracefulShutdownMs = 3000;
// The helper's own `MAX_LINE_BYTES`. It discards a longer line through its
// newline and answers nothing, so the host must not send one.
constexpr qsizetype helperLineLimitBytes = 4 * 1024;
// The helper's own `DDC_TIMEOUT`: the longest it would have waited for a
// `ddcutil` child before killing it. A helper that died without running its
// shutdown left no one holding that bound, so this is how long the host stays
// away from starting a replacement that would immediately detect displays.
constexpr int abandonedChildLifetimeMs = 20 * 1000;
} // namespace

ShellProvidersClient::ShellProvidersClient(QObject *parent)
    : QObject(parent)
{
    m_process.setProgram(qEnvironmentVariable(
        "CELESTINA_PROVIDER_ADAPTER_PATH",
        QStringLiteral(CELESTINA_PROVIDER_ADAPTER)
    ));
    m_process.setProcessChannelMode(QProcess::SeparateChannels);

    connect(
        &m_process,
        &QProcess::readyReadStandardOutput,
        this,
        &ShellProvidersClient::readStandardOutput
    );
    connect(
        &m_process,
        &QProcess::readyReadStandardError,
        this,
        &ShellProvidersClient::readStandardError
    );
    connect(&m_process, &QProcess::finished, this, &ShellProvidersClient::helperStopped);
    connect(&m_process, &QProcess::errorOccurred, this, &ShellProvidersClient::helperError);

    m_restartTimer.setSingleShot(true);
    connect(&m_restartTimer, &QTimer::timeout, this, &ShellProvidersClient::startHelper);

    m_restartDelayMs = initialRestartDelayMs;
    startHelper();
}

ShellProvidersClient::~ShellProvidersClient()
{
    m_stopping = true;
    m_restartTimer.stop();
    if (m_process.state() == QProcess::NotRunning)
        return;

    // Closing stdin is the helper's own shutdown signal: it drains its queue,
    // joins its worker and leaves. Terminating is the fallback for a helper
    // that does not.
    m_process.closeWriteChannel();
    if (m_process.waitForFinished(gracefulShutdownMs))
        return;

    m_process.terminate();
    if (!m_process.waitForFinished(gracefulShutdownMs)) {
        m_process.kill();
        m_process.waitForFinished(250);
    }
}

void ShellProvidersClient::startHelper()
{
    if (m_stopping || m_process.state() != QProcess::NotRunning)
        return;

    m_decoder.reset();
    m_tracedMediaVisual = false;
    // The spacing this exit earned has now been served.
    m_uncleanExit = false;
    m_process.start();
}

void ShellProvidersClient::scheduleRestart()
{
    if (m_stopping || m_restartTimer.isActive())
        return;

    // A helper that ran its own shutdown cancelled and reaped any DDC child it
    // owned, so the next one may start as soon as the backoff allows. One that
    // did not may have abandoned an active `ddcutil` child that nothing is
    // watching any more; starting a replacement immediately is what puts two
    // DDC conversations on one bus, which is the shape that preceded both
    // retained GPU losses. Waiting out the bound the helper would have applied
    // does not prove the orphan is gone — nobody reaps it now — but it removes
    // the overlap this shell creates automatically.
    const int delay = m_uncleanExit
        ? qMax(m_restartDelayMs, abandonedChildLifetimeMs)
        : m_restartDelayMs;
    m_restartDelayMs = qMin(maximumRestartDelayMs, m_restartDelayMs * 2);
    qInfo() << "Celestina will restart its provider helper in" << delay << "ms";
    m_restartTimer.start(delay);
}

void ShellProvidersClient::readStandardOutput()
{
    while (m_process.bytesAvailable() > 0) {
        const QByteArray chunk = m_process.read(readChunkBytes);
        if (chunk.isEmpty())
            break;

        const ProtocolDecoder::Result decoded = m_decoder.append(chunk);
        if (decoded.discardedOversizedLine) {
            qWarning() << "Celestina discarded an oversized provider helper line.";
            setUnavailable();
        }

        for (const QByteArray &line : decoded.lines)
            applyLine(line);
    }
}

void ShellProvidersClient::readStandardError()
{
    const QString message =
        QString::fromUtf8(m_process.readAllStandardError()).trimmed();
    if (!message.isEmpty())
        qWarning().noquote() << message;
}

void ShellProvidersClient::helperStopped(int exitCode, QProcess::ExitStatus exitStatus)
{
    readStandardOutput();
    readStandardError();
    m_decoder.reset();
    setUnavailable();
    if (!m_stopping) {
        qWarning() << "Celestina's provider helper stopped with code" << exitCode
                   << "and status" << exitStatus;
        // A crash exit means the helper never reached its own shutdown, so
        // whatever external child it held was abandoned rather than reaped.
        m_uncleanExit = exitStatus != QProcess::NormalExit;
        scheduleRestart();
    }
}

void ShellProvidersClient::helperError(QProcess::ProcessError error)
{
    qWarning().noquote()
        << (error == QProcess::FailedToStart
                ? "Celestina could not start its provider helper:"
                : "Celestina lost its provider helper:")
        << m_process.errorString();
    m_decoder.reset();
    setUnavailable();
    if (m_stopping)
        return;

    // A read/write failure can arrive while QProcess still reports Running. End
    // that unusable instance so `helperStopped` owns the restart.
    if (m_process.state() == QProcess::NotRunning) {
        scheduleRestart();
        return;
    }

    // SIGTERM, not SIGKILL: the helper answers it by cancelling and reaping any
    // DDC child it owns, which is the whole reason that shutdown path exists.
    // Killing outright would leave that child talking to the monitor bus with
    // nothing left to reap it. A helper that will not leave is still killed —
    // just not before it has been asked.
    m_process.terminate();
    QTimer::singleShot(gracefulShutdownMs, this, [this] {
        if (m_process.state() != QProcess::NotRunning) {
            qWarning() << "Celestina's provider helper ignored its termination "
                          "request and is being killed.";
            m_process.kill();
        }
    });
}

void ShellProvidersClient::applyLine(const QByteArray &line)
{
    const ProviderMessage message = parseProviderMessage(line);
    // Opt-in live diagnosis for the aggregate boundary. Provider payloads can
    // contain private text, so the trace names keys only and is silent unless
    // the launch environment explicitly requests it.
    const bool tracing = qEnvironmentVariableIsSet("CELESTINA_PROVIDER_TRACE");
    // One rule decides what a frame means for what is on screen; this only
    // obeys it. See `FrameEffect` for why an unreadable frame changes nothing.
    switch (effectOf(message)) {
    case FrameEffect::Ignore:
        qWarning().noquote()
            << "Celestina rejected a provider helper frame:" << message.error;
        return;
    case FrameEffect::Answer: {
        bool parsed = false;
        const quint64 requestId = message.requestId.toULongLong(&parsed);
        if (!parsed) {
            qWarning() << "Celestina rejected an unusable provider request id.";
            return;
        }
        if (message.state == QStringLiteral("failed")) {
            qWarning().noquote()
                << "Celestina's provider request failed:" << message.reason;
        }
        emit commandResult(requestId, message.state, message.reason);
        return;
    }
    case FrameEffect::Replace:
        break;
    }

    const bool becameAvailable = !m_available;
    m_available = true;
    m_restartDelayMs = initialRestartDelayMs;
    const bool stateChanged = m_states.apply(message);
    if (tracing && stateChanged) {
        qInfo().noquote() << "Celestina provider frame"
                          << message.generation << "keys"
                          << message.providers.keys().join(u',');
    }
    if (stateChanged || becameAvailable)
        emit changed();

    if (tracing && !m_tracedMediaVisual
        && message.providers.contains(QStringLiteral("media"))) {
        m_tracedMediaVisual = true;
        QTimer::singleShot(0, this, [] {
            for (QWindow *window : QGuiApplication::allWindows()) {
                QObject *media = window->findChild<QObject *>(
                    QStringLiteral("celestina-panel-media")
                );
                if (!media)
                    continue;
                QObject *container = media->parent();
                qInfo().noquote()
                    << "Celestina media visual"
                    << window->objectName()
                    << "hasPlayer" << media->property("hasPlayer")
                    << "visible" << media->property("visible")
                    << "x" << media->property("x")
                    << "width" << media->property("width")
                    << "implicitWidth" << media->property("implicitWidth")
                    << "containerWidth"
                    << (container ? container->property("width") : QVariant());
            }
        });
    }
}

void ShellProvidersClient::setUnavailable()
{
    const bool dropped = m_states.clear();
    if (!m_available && !dropped)
        return;

    m_available = false;
    emit changed();
}

qulonglong ShellProvidersClient::sendCommand(
    const QString &provider,
    const QString &verb,
    const QVariantMap &options
)
{
    if (m_process.state() != QProcess::Running)
        return 0;

    const quint64 requestId = ++m_lastRequestId;
    QJsonObject command {
        {QStringLiteral("id"), QString::number(requestId)},
        {QStringLiteral("provider"), provider},
        {QStringLiteral("verb"), verb},
        {QStringLiteral("options"), QJsonObject::fromVariantMap(options)},
    };

    QByteArray line = QJsonDocument(command).toJson(QJsonDocument::Compact);
    line.append('\n');
    // The helper discards a line past its own limit and cannot name the request
    // it discarded, so the caller would hold a live id nothing will ever answer.
    // Refusing here is what turns that into a visible failure. The id is spent
    // either way: ids are never reused.
    if (line.size() > helperLineLimitBytes) {
        qWarning() << "Celestina refused to send an oversized provider command of"
                   << line.size() << "bytes.";
        return 0;
    }
    if (m_process.write(line) != line.size()) {
        qWarning() << "Celestina could not send a provider command.";
        return 0;
    }
    return requestId;
}
