#include "niriclient.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QVariantMap>

namespace {
constexpr qsizetype maxAdapterMessageBytes = 1024 * 1024;
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
        [this](QProcess::ProcessError error) {
            if (error == QProcess::FailedToStart) {
                qWarning().noquote()
                    << "Celestina could not start its Niri adapter:"
                    << m_process.errorString();
                setUnavailable();
            }
        }
    );

    m_process.start();
}

void NiriClient::readStandardOutput()
{
    m_outputBuffer += m_process.readAllStandardOutput();
    if (m_outputBuffer.size() > maxAdapterMessageBytes
        && !m_outputBuffer.contains('\n')) {
        qWarning() << "Celestina discarded an oversized Niri adapter message.";
        m_outputBuffer.clear();
        setUnavailable();
        return;
    }

    qsizetype newline = -1;
    while ((newline = m_outputBuffer.indexOf('\n')) >= 0) {
        const QByteArray line = m_outputBuffer.left(newline);
        m_outputBuffer.remove(0, newline + 1);
        if (line.size() > maxAdapterMessageBytes) {
            qWarning() << "Celestina discarded an oversized Niri adapter line.";
            setUnavailable();
            continue;
        }
        if (!line.trimmed().isEmpty())
            applyMessage(line);
    }
}

void NiriClient::readStandardError()
{
    const QString message = QString::fromUtf8(m_process.readAllStandardError()).trimmed();
    if (!message.isEmpty())
        qWarning().noquote() << message;
}

void NiriClient::adapterStopped()
{
    readStandardOutput();
    readStandardError();
    setUnavailable();
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
        if (!value.isObject())
            continue;

        const QJsonObject workspace = value.toObject();
        const QString output = boundedString(
            workspace.value(QStringLiteral("output")), maxLabelLength
        );
        const int index = workspace.value(QStringLiteral("index")).toInt();
        if (output.isEmpty() || index < 1 || index > 255)
            continue;

        QVariantMap item;
        item.insert(QStringLiteral("index"), index);
        item.insert(
            QStringLiteral("label"),
            boundedString(workspace.value(QStringLiteral("label")), maxLabelLength)
        );
        item.insert(QStringLiteral("output"), output);
        item.insert(
            QStringLiteral("active"),
            workspace.value(QStringLiteral("active")).toBool()
        );
        item.insert(
            QStringLiteral("focused"),
            workspace.value(QStringLiteral("focused")).toBool()
        );
        item.insert(
            QStringLiteral("urgent"),
            workspace.value(QStringLiteral("urgent")).toBool()
        );
        item.insert(
            QStringLiteral("activeWindowTitle"),
            boundedString(
                workspace.value(QStringLiteral("active_window_title")),
                maxTitleLength
            )
        );
        nextWorkspaces.append(item);
    }

    if (!m_available || m_workspaces != nextWorkspaces) {
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
