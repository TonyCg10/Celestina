#include "niriclient.h"

#include <cmath>

#include <utility>

#include <QHash>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QVariantMap>

namespace {
constexpr qint64 adapterReadChunkBytes = 64 * 1024;
constexpr int initialRestartDelayMs = 250;
constexpr int maximumRestartDelayMs = 10 * 1000;
// Requests are short-lived; this is only how often their deadlines are
// checked, so the panel never holds a stale pending pill for long.
constexpr int requestSweepIntervalMs = 100;
// A screenshot or action request has the same deadline as a workspace focus
// request, and deliberately the same one rather than a second policy: they
// travel the same pipe to the same compositor, and the adapter answers all of
// them from one worker. If that worker wedges while its event stream stays
// alive, nothing else would ever answer these — the request would sit in the
// table until the queue filled and every later action failed as "queue is
// full", with no failure reported for any of the requests that caused it.
constexpr qint64 pendingRequestTimeoutMs =
    WorkspaceFocusRequests::Timings {}.pendingTimeoutMs;
constexpr qsizetype maxWorkspaceCount = 512;
constexpr qsizetype maxLabelLength = 128;
constexpr qsizetype maxTitleLength = 512;
// Workspace ids and request ids both travel as opaque `u64` decimals in text
// form, because JSON numbers would reach this parser as doubles. Twenty digits
// is the widest such value that exists.
constexpr qsizetype maxIdentifierLength = 20;

QString boundedString(const QJsonValue &value, qsizetype maximum)
{
    const QString string = value.toString();
    return string.left(maximum);
}

bool isDecimalIdentifier(const QJsonValue &value)
{
    if (!value.isString())
        return false;

    const QString id = value.toString();
    if (id.isEmpty() || id.size() > maxIdentifierLength)
        return false;

    for (const QChar character : id) {
        if (character < u'0' || character > u'9')
            return false;
    }
    return true;
}

// The ids whose deadline has passed. They are collected before anything is
// removed or reported, because failing a request can run a listener that sends
// the next one into the very table being walked.
QList<quint64> expiredRequests(const QHash<quint64, qint64> &startedMs, qint64 nowMs)
{
    QList<quint64> expired;
    for (auto entry = startedMs.cbegin(); entry != startedMs.cend(); ++entry) {
        if (nowMs - entry.value() >= pendingRequestTimeoutMs)
            expired.append(entry.key());
    }
    return expired;
}
} // namespace

NiriClient::NiriClient(QObject *parent)
    : QObject(parent)
{
    m_process.setProgram(qEnvironmentVariable(
        "CELESTINA_NIRI_ADAPTER_PATH",
        QStringLiteral(CELESTINA_NIRI_ADAPTER)
    ));
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

    m_requestTimer.setInterval(requestSweepIntervalMs);
    connect(&m_requestTimer, &QTimer::timeout, this, &NiriClient::expireRequests);

    m_clock.start();
    m_restartDelayMs = initialRestartDelayMs;
    startAdapter();
}

NiriClient::~NiriClient()
{
    m_stopping = true;
    m_restartTimer.stop();
    m_requestTimer.stop();
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
    // Every request belongs to exactly one helper process. A result that
    // arrives from an older one answers nothing here.
    ++m_generation;
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

        const ProtocolDecoder::Result decoded = m_decoder.append(chunk);
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
    if (kind == QStringLiteral("request")) {
        if (!applyRequestResult(root))
            setUnavailable();
        return;
    }
    if (kind != QStringLiteral("snapshot")
        || !root.value(QStringLiteral("workspaces")).isArray()) {
        qWarning() << "Celestina ignored an unknown Niri adapter message.";
        setUnavailable();
        return;
    }

    if (!applySnapshot(root))
        setUnavailable();
}

bool NiriClient::applySnapshot(const QJsonObject &root)
{
    const QJsonArray workspaces = root.value(QStringLiteral("workspaces")).toArray();
    if (workspaces.size() > maxWorkspaceCount) {
        qWarning() << "Celestina discarded an oversized Niri workspace snapshot.";
        return false;
    }

    QVariantList nextWorkspaces;
    nextWorkspaces.reserve(workspaces.size());
    for (const QJsonValue &value : workspaces) {
        if (!value.isObject()) {
            qWarning() << "Celestina rejected an invalid Niri workspace snapshot.";
            return false;
        }

        const QJsonObject workspace = value.toObject();
        const QJsonValue idValue = workspace.value(QStringLiteral("id"));
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
        if (!isDecimalIdentifier(idValue)
            || !outputValue.isString() || !labelValue.isString()
            || !indexValue.isDouble() || !std::isfinite(numericIndex)
            || std::floor(numericIndex) != numericIndex
            || !activeValue.isBool() || !focusedValue.isBool()
            || !urgentValue.isBool()
            || (!titleValue.isNull() && !titleValue.isString())
            || output.isEmpty() || outputValue.toString().size() > maxLabelLength
            || labelValue.toString().isEmpty()
            || numericIndex < 1 || numericIndex > 255) {
            qWarning() << "Celestina rejected an invalid Niri workspace snapshot.";
            return false;
        }
        const int index = static_cast<int>(numericIndex);

        QVariantMap item;
        item.insert(QStringLiteral("id"), idValue.toString());
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

    // A request is confirmed by the compositor reporting the workspace it
    // named as active on the output it named — never by the mere arrival of
    // the next snapshot.
    QHash<QString, int> activeByOutput;
    for (const QVariant &value : std::as_const(nextWorkspaces)) {
        const QVariantMap item = value.toMap();
        if (item.value(QStringLiteral("active")).toBool()) {
            activeByOutput.insert(
                item.value(QStringLiteral("output")).toString(),
                item.value(QStringLiteral("index")).toInt()
            );
        }
    }

    const bool becameAvailable = !m_available;
    m_available = true;
    m_restartDelayMs = initialRestartDelayMs;
    m_snapshot = nextWorkspaces;
    m_requests.applyActive(activeByOutput, nowMs());

    const bool listChanged = rebuildWorkspaces();
    scheduleRequestOutcomes();
    if (listChanged || becameAvailable)
        emit changed();
    return true;
}

bool NiriClient::applyRequestResult(const QJsonObject &root)
{
    const QJsonValue idValue = root.value(QStringLiteral("id"));
    const QJsonValue stateValue = root.value(QStringLiteral("state"));
    if (!isDecimalIdentifier(idValue) || !stateValue.isString()) {
        qWarning() << "Celestina rejected an invalid Niri request result.";
        return false;
    }

    const QString state = stateValue.toString();
    const bool accepted = state == QStringLiteral("accepted");
    if (!accepted && state != QStringLiteral("failed")) {
        qWarning() << "Celestina rejected an unknown Niri request state.";
        return false;
    }

    bool parsed = false;
    const quint64 requestId = idValue.toString().toULongLong(&parsed);
    if (!parsed) {
        qWarning() << "Celestina rejected an unusable Niri request id.";
        return false;
    }

    const QString reason =
        boundedString(root.value(QStringLiteral("reason")), maxTitleLength);
    if (m_actionRequests.contains(requestId)) {
        // Niri answered the action itself, so its answer is the outcome — this
        // is the compositor reporting what it did, not a helper reporting that
        // it will try. A refusal leaves through the same path an expiry does,
        // so the requester cannot tell the two apart by shape.
        if (!accepted) {
            failActionRequests({requestId}, reason);
            return true;
        }

        m_actionRequests.remove(requestId);
        emit actionFinished(requestId, QStringLiteral("confirmed"), reason);
        return true;
    }

    if (m_screenshotRequests.contains(requestId)) {
        if (accepted)
            m_screenshotRequests.remove(requestId);
        else
            failScreenshotRequests({requestId}, reason);
        return true;
    }

    if (!accepted) {
        qWarning().noquote()
            << "Celestina's workspace focus request failed:" << reason;
    }

    if (m_requests.acknowledge(requestId, m_generation, accepted, nowMs())
        && rebuildWorkspaces()) {
        emit changed();
    }
    scheduleRequestOutcomes();
    return true;
}

qulonglong NiriClient::requestWorkspaceFocus(const QString &output, int index)
{
    if (!m_available || m_process.state() != QProcess::Running)
        return false;

    QString workspaceId;
    for (const QVariant &value : std::as_const(m_snapshot)) {
        const QVariantMap item = value.toMap();
        if (item.value(QStringLiteral("output")).toString() == output
            && item.value(QStringLiteral("index")).toInt() == index) {
            workspaceId = item.value(QStringLiteral("id")).toString();
            break;
        }
    }
    // The panel may only request what the compositor last reported. An unknown
    // workspace is a stale click, not a request worth sending.
    if (workspaceId.isEmpty())
        return false;

    const quint64 requestId = m_lastRequestId + 1;
    if (!m_requests.begin(requestId, m_generation, output, index, nowMs()))
        return false;
    m_lastRequestId = requestId;

    const QJsonObject command {
        {QStringLiteral("kind"), QStringLiteral("focus-workspace")},
        {QStringLiteral("id"), QString::number(requestId)},
        {QStringLiteral("workspace"), workspaceId},
    };
    const QByteArray line =
        QJsonDocument(command).toJson(QJsonDocument::Compact) + '\n';

    const bool sent = m_process.write(line) == line.size();
    if (!sent) {
        qWarning() << "Celestina could not send a workspace focus request.";
        m_requests.acknowledge(requestId, m_generation, false, nowMs());
    }
    startRequestSweep();
    if (rebuildWorkspaces())
        emit changed();
    scheduleRequestOutcomes();
    return sent ? requestId : 0;
}

void NiriClient::expireRequests()
{
    const qint64 now = nowMs();
    if (m_requests.expire(now) && rebuildWorkspaces())
        emit changed();
    scheduleRequestOutcomes();

    // A screenshot or an action that ran out of time is reported through the
    // same path a compositor refusal takes: whoever asked hears "failed" once,
    // never silence, and the entry stops occupying the adapter's queue budget.
    failScreenshotRequests(
        expiredRequests(m_screenshotRequests, now),
        tr("the Niri helper did not answer in time")
    );
    failActionRequests(
        expiredRequests(m_actionRequests, now),
        tr("the Niri helper did not answer in time")
    );

    if (m_requests.isEmpty() && m_screenshotRequests.isEmpty()
        && m_actionRequests.isEmpty()) {
        m_requestTimer.stop();
    }
}

void NiriClient::failScreenshotRequests(
    const QList<quint64> &requestIds,
    const QString &reason
)
{
    bool refused = false;
    for (const quint64 requestId : requestIds)
        refused = m_screenshotRequests.remove(requestId) > 0 || refused;
    if (!refused)
        return;

    // A screenshot has no per-request outcome to report — the panel only knows
    // that the capture it asked for is not happening — so one refusal covers
    // however many were in flight.
    qWarning().noquote() << "Celestina's screenshot request failed:" << reason;
    emit screenshotFailed(reason);
}

void NiriClient::failActionRequests(
    const QList<quint64> &requestIds,
    const QString &reason
)
{
    for (const quint64 requestId : requestIds) {
        if (m_actionRequests.remove(requestId) == 0)
            continue;

        emit actionFinished(requestId, QStringLiteral("failed"), reason);
    }
}

void NiriClient::startRequestSweep()
{
    if (!m_requestTimer.isActive())
        m_requestTimer.start();
}

void NiriClient::scheduleRequestOutcomes()
{
    if (m_outcomesQueued)
        return;

    // Transitions leave on the next event-loop turn, never from inside the
    // mutation that produced them: a listener then sees a settled client, and
    // the caller of a request already holds its id when "pending" arrives.
    m_outcomesQueued = true;
    QTimer::singleShot(0, this, &NiriClient::drainRequestOutcomes);
}

void NiriClient::drainRequestOutcomes()
{
    m_outcomesQueued = false;
    const QList<WorkspaceFocusRequests::Outcome> outcomes =
        m_requests.takeOutcomes();
    for (const WorkspaceFocusRequests::Outcome &outcome : outcomes)
        emit focusRequestChanged(outcome.requestId, outcome.state);
}

bool NiriClient::rebuildWorkspaces()
{
    QVariantList published;
    published.reserve(m_snapshot.size());
    for (const QVariant &value : std::as_const(m_snapshot)) {
        QVariantMap item = value.toMap();
        const QString output = item.value(QStringLiteral("output")).toString();
        const int index = item.value(QStringLiteral("index")).toInt();
        // The Niri id is this adapter's handle for a request. QML consumes
        // states, never protocol identifiers.
        item.remove(QStringLiteral("id"));
        item.insert(
            QStringLiteral("requestState"),
            m_requests.stateFor(output, index)
        );
        published.append(item);
    }

    if (m_workspaces == published)
        return false;

    m_workspaces = published;
    return true;
}

qulonglong NiriClient::sendRequest(const QString &kind)
{
    if (m_process.state() != QProcess::Running)
        return 0;

    const quint64 requestId = ++m_lastRequestId;
    QByteArray line = QJsonDocument(QJsonObject {
                                        {QStringLiteral("kind"), kind},
                                        {QStringLiteral("id"), QString::number(requestId)},
                                    })
                          .toJson(QJsonDocument::Compact);
    line.append('\n');

    if (m_process.write(line) != line.size())
        return 0;

    return requestId;
}

qulonglong NiriClient::requestScreenshot()
{
    const qulonglong requestId = sendRequest(QStringLiteral("screenshot"));
    if (requestId == 0) {
        qWarning() << "Celestina could not send a screenshot request.";
        emit screenshotFailed(tr("no se pudo pedir la captura"));
        return 0;
    }

    m_screenshotRequests.insert(requestId, nowMs());
    startRequestSweep();
    return requestId;
}

qulonglong NiriClient::requestDisplaysOff()
{
    const qulonglong requestId = sendRequest(QStringLiteral("power-off-monitors"));
    if (requestId == 0)
        return 0;

    m_actionRequests.insert(requestId, nowMs());
    startRequestSweep();
    return requestId;
}

qulonglong NiriClient::requestLogOut()
{
    const qulonglong requestId = sendRequest(QStringLiteral("quit"));
    if (requestId == 0)
        return 0;

    m_actionRequests.insert(requestId, nowMs());
    startRequestSweep();
    return requestId;
}

void NiriClient::setUnavailable()
{
    // A helper that went away can no longer answer a screenshot or an action,
    // and an unanswered request must not wait forever for a result that is not
    // coming. Whoever asked is told it failed rather than being left waiting on
    // a compositor that is no longer being listened to.
    failScreenshotRequests(
        m_screenshotRequests.keys(),
        tr("el ayudante de Niri no está disponible")
    );
    failActionRequests(
        m_actionRequests.keys(),
        tr("the Niri helper is unavailable")
    );

    // Nothing in flight can still be confirmed once the compositor's state is
    // gone; the requests fail rather than waiting out their timeout.
    const bool requestsChanged = m_requests.failAll(nowMs());
    if (!m_available && m_snapshot.isEmpty() && !requestsChanged)
        return;

    m_available = false;
    m_snapshot.clear();
    rebuildWorkspaces();
    scheduleRequestOutcomes();
    emit changed();
}
