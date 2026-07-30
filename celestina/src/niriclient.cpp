#include "niriclient.h"

#include <cmath>

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QVariantMap>

namespace {
constexpr qint64 adapterReadChunkBytes = 64 * 1024;
constexpr int initialRestartDelayMs = 250;
constexpr int maximumRestartDelayMs = 10 * 1000;
constexpr qsizetype maxWorkspaceCount = 512;
constexpr qsizetype maxLabelLength = 128;
constexpr qsizetype maxTitleLength = 512;

QString boundedString(const QJsonValue &value, qsizetype maximum)
{
    const QString string = value.toString();
    return string.left(maximum);
}
} // namespace

NiriClient::NiriClient(QObject *parent)
    : QObject(parent)
{
    m_process.setProgram(QStringLiteral(CELESTINA_NIRI_ADAPTER));
    m_process.setProcessChannelMode(QProcess::SeparateChannels);

    connect(
        &m_process,
        &QProcess::readyReadStandardOutput,
        this,
        &NiriClient::readStandardOutput
    );
    connect(
        &m_process,
        &QProcess::readyReadStandardError,
        this,
        &NiriClient::readStandardError
    );
    connect(
        &m_process,
        &QProcess::finished,
        this,
        &NiriClient::adapterStopped
    );
    connect(
        &m_process,
        &QProcess::errorOccurred,
        this,
        &NiriClient::adapterError
    );
    m_restartTimer.setSingleShot(true);
    connect(&m_restartTimer, &QTimer::timeout, this, &NiriClient::startAdapter);

    m_restartDelayMs = initialRestartDelayMs;
    startAdapter();
}

NiriClient::~NiriClient()
{
    m_stopping = true;
    m_restartTimer.stop();
    if (m_process.state() == QProcess::NotRunning)
        return;

    m_process.terminate();
    if (!m_process.waitForFinished(250)) {
        m_process.kill();
        m_process.waitForFinished(250);
    }
}

void NiriClient::startAdapter()
{
    if (m_stopping || m_process.state() != QProcess::NotRunning)
        return;

    m_decoder.reset();
    m_process.start();
}

void NiriClient::scheduleRestart()
{
    if (m_stopping || m_restartTimer.isActive())
        return;

    const int delay = m_restartDelayMs;
    m_restartDelayMs = qMin(maximumRestartDelayMs, m_restartDelayMs * 2);
    qInfo() << "Celestina will restart its Niri adapter in" << delay << "ms";
    m_restartTimer.start(delay);
}

void NiriClient::readStandardOutput()
{
    while (m_process.bytesAvailable() > 0) {
        const QByteArray chunk = m_process.read(adapterReadChunkBytes);
        if (chunk.isEmpty())
            break;

        const NiriProtocolDecoder::Result decoded = m_decoder.append(chunk);
        if (decoded.discardedOversizedLine) {
            qWarning() << "Celestina discarded an oversized Niri adapter line.";
            setUnavailable();
        }

        for (const QByteArray &line : decoded.lines)
            applyMessage(line);
    }
}

void NiriClient::readStandardError()
{
    const QString message = QString::fromUtf8(m_process.readAllStandardError()).trimmed();
    if (!message.isEmpty())
        qWarning().noquote() << message;
}

void NiriClient::adapterStopped(int exitCode, QProcess::ExitStatus exitStatus)
{
    readStandardOutput();
    readStandardError();
    m_decoder.reset();
    setUnavailable();
    if (!m_stopping) {
        qWarning() << "Celestina Niri adapter stopped with code" << exitCode
                   << "and status" << exitStatus;
        scheduleRestart();
    }
}

void NiriClient::adapterError(QProcess::ProcessError error)
{
    qWarning().noquote()
        << (error == QProcess::FailedToStart
                ? "Celestina could not start its Niri adapter:"
                : "Celestina lost its Niri adapter process:")
        << m_process.errorString();
    m_decoder.reset();
    setUnavailable();
    if (m_stopping)
        return;

    // A read/write/unknown process failure can arrive while QProcess still
    // reports Running. End that unusable instance so adapterStopped owns the
    // restart; FailedToStart and already-stopped failures restart directly.
    if (m_process.state() == QProcess::NotRunning)
        scheduleRestart();
    else
        m_process.kill();
}

void NiriClient::applyMessage(const QByteArray &line)
{
    QJsonParseError parseError;
    const QJsonDocument document = QJsonDocument::fromJson(line, &parseError);
    if (parseError.error != QJsonParseError::NoError || !document.isObject()) {
        qWarning().noquote()
            << "Celestina ignored invalid Niri adapter JSON:"
            << parseError.errorString();
        setUnavailable();
        return;
    }

    const QJsonObject root = document.object();
    const QString kind = root.value(QStringLiteral("kind")).toString();
    if (kind == QStringLiteral("unavailable")) {
        setUnavailable();
        return;
    }
    if (kind != QStringLiteral("snapshot")
        || !root.value(QStringLiteral("workspaces")).isArray()) {
        qWarning() << "Celestina ignored an unknown Niri adapter message.";
        setUnavailable();
        return;
    }

    const QJsonArray workspaces = root.value(QStringLiteral("workspaces")).toArray();
    if (workspaces.size() > maxWorkspaceCount) {
        qWarning() << "Celestina discarded an oversized Niri workspace snapshot.";
        setUnavailable();
        return;
    }

    QVariantList nextWorkspaces;
    nextWorkspaces.reserve(workspaces.size());
    for (const QJsonValue &value : workspaces) {
        if (!value.isObject()) {
            qWarning() << "Celestina rejected an invalid Niri workspace snapshot.";
            setUnavailable();
            return;
        }

        const QJsonObject workspace = value.toObject();
        const QJsonValue outputValue = workspace.value(QStringLiteral("output"));
        const QJsonValue labelValue = workspace.value(QStringLiteral("label"));
        const QJsonValue indexValue = workspace.value(QStringLiteral("index"));
        const QJsonValue activeValue = workspace.value(QStringLiteral("active"));
        const QJsonValue focusedValue = workspace.value(QStringLiteral("focused"));
        const QJsonValue urgentValue = workspace.value(QStringLiteral("urgent"));
        const QJsonValue titleValue = workspace.value(
            QStringLiteral("active_window_title")
        );
        const QString output = boundedString(outputValue, maxLabelLength);
        const double numericIndex = indexValue.toDouble();
        if (!outputValue.isString() || !labelValue.isString()
            || !indexValue.isDouble() || !std::isfinite(numericIndex)
            || std::floor(numericIndex) != numericIndex
            || !activeValue.isBool() || !focusedValue.isBool()
            || !urgentValue.isBool()
            || (!titleValue.isNull() && !titleValue.isString())
            || output.isEmpty() || outputValue.toString().size() > maxLabelLength
            || labelValue.toString().isEmpty()
            || numericIndex < 1 || numericIndex > 255) {
            qWarning() << "Celestina rejected an invalid Niri workspace snapshot.";
            setUnavailable();
            return;
        }
        const int index = static_cast<int>(numericIndex);

        QVariantMap item;
        item.insert(QStringLiteral("index"), index);
        item.insert(
            QStringLiteral("label"),
            boundedString(labelValue, maxLabelLength)
        );
        item.insert(QStringLiteral("output"), output);
        item.insert(
            QStringLiteral("active"),
            activeValue.toBool()
        );
        item.insert(
            QStringLiteral("focused"),
            focusedValue.toBool()
        );
        item.insert(
            QStringLiteral("urgent"),
            urgentValue.toBool()
        );
        item.insert(
            QStringLiteral("activeWindowTitle"),
            boundedString(
                titleValue, maxTitleLength
            )
        );
        nextWorkspaces.append(item);
    }

    if (!m_available || m_workspaces != nextWorkspaces) {
        m_restartDelayMs = initialRestartDelayMs;
        m_available = true;
        m_workspaces = nextWorkspaces;
        emit changed();
    }
}

void NiriClient::setUnavailable()
{
    if (!m_available && m_workspaces.isEmpty())
        return;

    m_available = false;
    m_workspaces.clear();
    emit changed();
}
