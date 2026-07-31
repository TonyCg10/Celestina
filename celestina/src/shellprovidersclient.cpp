#include "shellprovidersclient.h"

#include <QDebug>
#include <QJsonDocument>
#include <QJsonObject>

namespace {
constexpr qint64 readChunkBytes = 64 * 1024;
constexpr int initialRestartDelayMs = 250;
constexpr int maximumRestartDelayMs = 10 * 1000;
} // namespace

ShellProvidersClient::ShellProvidersClient(QObject *parent)
    : QObject(parent)
{
    m_process.setProgram(QStringLiteral(CELESTINA_PROVIDER_ADAPTER));
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
    if (m_process.waitForFinished(250))
        return;

    m_process.terminate();
    if (!m_process.waitForFinished(250)) {
        m_process.kill();
        m_process.waitForFinished(250);
    }
}

void ShellProvidersClient::startHelper()
{
    if (m_stopping || m_process.state() != QProcess::NotRunning)
        return;

    m_decoder.reset();
    m_process.start();
}

void ShellProvidersClient::scheduleRestart()
{
    if (m_stopping || m_restartTimer.isActive())
        return;

    const int delay = m_restartDelayMs;
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
    if (m_process.state() == QProcess::NotRunning)
        scheduleRestart();
    else
        m_process.kill();
}

void ShellProvidersClient::applyLine(const QByteArray &line)
{
    const ProviderMessage message = parseProviderMessage(line);
    switch (message.kind) {
    case ProviderMessage::Kind::Invalid:
        qWarning().noquote()
            << "Celestina rejected a provider helper frame:" << message.error;
        setUnavailable();
        return;
    case ProviderMessage::Kind::Result: {
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
    case ProviderMessage::Kind::Providers:
        break;
    }

    const bool becameAvailable = !m_available;
    m_available = true;
    m_restartDelayMs = initialRestartDelayMs;
    if (m_states.apply(message) || becameAvailable)
        emit changed();
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
    if (m_process.write(line) != line.size()) {
        qWarning() << "Celestina could not send a provider command.";
        return 0;
    }
    return requestId;
}
