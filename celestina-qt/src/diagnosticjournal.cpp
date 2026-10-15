#include "diagnosticjournal.h"

#include <algorithm>
#include <QCoreApplication>
#include <QDateTime>
#include <QDebug>
#include <QDir>
#include <QElapsedTimer>
#include <QFileInfo>
#include <QStandardPaths>

#include <fcntl.h>
#include <unistd.h>

#include <chrono>

namespace {

// The same numbers `celestina-shell-core::diagnostics` declares. They are
// repeated rather than shared because this process does not link that crate;
// see the header for why that boundary is real. A change on either side is a
// change to both.
constexpr int schemaVersion = 1;
constexpr int maxLineBytes = 4 * 1024;
constexpr qint64 maxFileBytes = 4LL * 1024 * 1024;
constexpr int maxFiles = 8;
constexpr int maxQueue = 4096;
constexpr int maxTextChars = 160;
constexpr int maxFields = 24;
constexpr int drainDeadlineMillis = 1500;
constexpr int reopenAfterMillis = 30000;

const char *levelToken(DiagnosticJournal::Level level)
{
    switch (level) {
    case DiagnosticJournal::Level::Trace: return "trace";
    case DiagnosticJournal::Level::Debug: return "debug";
    case DiagnosticJournal::Level::Info: return "info";
    case DiagnosticJournal::Level::Warn: return "warn";
    case DiagnosticJournal::Level::Error: return "error";
    case DiagnosticJournal::Level::Critical: return "critical";
    }
    return "info";
}

bool mustFlush(DiagnosticJournal::Level level)
{
    return level == DiagnosticJournal::Level::Critical;
}

bool mirrored(DiagnosticJournal::Level level)
{
    return level >= DiagnosticJournal::Level::Warn;
}

// Escaped exactly as the crate escapes: every control character, so no
// producer's text can inject a second line into a file a script will parse.
QByteArray jsonString(const QString &value)
{
    QByteArray out;
    out.append('"');
    for (const QChar character : value) {
        const ushort code = character.unicode();
        switch (code) {
        case '"': out.append("\\\""); break;
        case '\\': out.append("\\\\"); break;
        case '\n': out.append("\\n"); break;
        case '\r': out.append("\\r"); break;
        case '\t': out.append("\\t"); break;
        default:
            if (code < 0x20)
                out.append(QStringLiteral("\\u%1").arg(code, 4, 16, QLatin1Char('0')).toUtf8());
            else
                out.append(QString(character).toUtf8());
        }
    }
    out.append('"');
    return out;
}

QString bounded(const QString &value)
{
    return value.size() > maxTextChars ? value.left(maxTextChars) : value;
}

quint64 nowNanos()
{
    const auto since = std::chrono::system_clock::now().time_since_epoch();
    return static_cast<quint64>(
        std::chrono::duration_cast<std::chrono::nanoseconds>(since).count()
    );
}

quint64 nowMillis()
{
    const auto since = std::chrono::steady_clock::now().time_since_epoch();
    return static_cast<quint64>(
        std::chrono::duration_cast<std::chrono::milliseconds>(since).count()
    );
}

// Keeps a file name to characters that cannot escape a directory or confuse the
// report script that reads the bundle later.
QString sanitized(const QString &value)
{
    QString cleaned;
    for (const QChar character : value) {
        if (character.isLetterOrNumber() || character == QLatin1Char('-')
            || character == QLatin1Char('_'))
            cleaned.append(character);
        else
            cleaned.append(QLatin1Char('_'));
        if (cleaned.size() >= 64)
            break;
    }
    return cleaned.isEmpty() ? QStringLiteral("unknown") : cleaned;
}

} // namespace

DiagnosticJournal::Record::Record(Level level, QString event)
    : m_level(level)
    , m_event(bounded(std::move(event)))
{
}

DiagnosticJournal::Record &DiagnosticJournal::Record::text(const QString &key, const QString &value)
{
    if (m_fieldCount >= maxFields)
        return *this;
    ++m_fieldCount;
    m_fields.append(',');
    m_fields.append(jsonString(bounded(key)));
    m_fields.append(':');
    m_fields.append(jsonString(bounded(value)));
    return *this;
}

DiagnosticJournal::Record &DiagnosticJournal::Record::number(const QString &key, qint64 value)
{
    if (m_fieldCount >= maxFields)
        return *this;
    ++m_fieldCount;
    m_fields.append(',');
    m_fields.append(jsonString(bounded(key)));
    m_fields.append(':');
    m_fields.append(QByteArray::number(value));
    return *this;
}

DiagnosticJournal::Record &
DiagnosticJournal::Record::unsigned_number(const QString &key, quint64 value)
{
    if (m_fieldCount >= maxFields)
        return *this;
    ++m_fieldCount;
    m_fields.append(',');
    m_fields.append(jsonString(bounded(key)));
    m_fields.append(':');
    m_fields.append(QByteArray::number(value));
    return *this;
}

DiagnosticJournal::Record &DiagnosticJournal::Record::millis(const QString &key, quint64 value)
{
    return unsigned_number(key, value);
}

DiagnosticJournal::Record &DiagnosticJournal::Record::flag(const QString &key, bool value)
{
    if (m_fieldCount >= maxFields)
        return *this;
    ++m_fieldCount;
    m_fields.append(',');
    m_fields.append(jsonString(bounded(key)));
    m_fields.append(value ? ":true" : ":false");
    return *this;
}

DiagnosticJournal::Record &
DiagnosticJournal::Record::redacted(const QString &key, const QString &value)
{
    // Two numbers and no text, exactly as `diagnostics::Redaction` does. There
    // is deliberately no digest: a hash of a short title is guessable, so it
    // would invite the brute force it appears to prevent.
    unsigned_number(key + QStringLiteral("_chars"), static_cast<quint64>(value.size()));
    return unsigned_number(
        key + QStringLiteral("_bytes"),
        static_cast<quint64>(value.toUtf8().size())
    );
}

DiagnosticJournal &DiagnosticJournal::instance()
{
    // Intentionally process-lifetime. A writer detached after the bounded
    // shutdown deadline must retain valid storage until the kernel ends the
    // process; a static destructor would otherwise race that writer.
    static DiagnosticJournal *journal = new DiagnosticJournal;
    return *journal;
}

QString DiagnosticJournal::runId()
{
    // Generated once per host invocation and then stable, so the value handed
    // to a helper started later is the same one the first line carried.
    static const QString identifier = [] {
        const QString inherited = qEnvironmentVariable("CELESTINA_RUN_ID");
        if (!inherited.trimmed().isEmpty())
            return bounded(inherited.trimmed());
        return QStringLiteral("%1-%2")
            .arg(nowNanos(), 0, 16)
            .arg(static_cast<quint64>(QCoreApplication::applicationPid()), 0, 16);
    }();
    return identifier;
}

void DiagnosticJournal::exportRunId()
{
    qputenv("CELESTINA_RUN_ID", runId().toUtf8());
}

void DiagnosticJournal::open(const QString &component)
{
    if (m_open)
        return;
    m_open = true;
    m_component = sanitized(component);
    m_mirror = qEnvironmentVariable("CELESTINA_JOURNAL_MIRROR") != QStringLiteral("0")
        && qEnvironmentVariable("CELESTINA_JOURNAL_MIRROR") != QStringLiteral("off")
        && qEnvironmentVariable("CELESTINA_JOURNAL_MIRROR") != QStringLiteral("false");

    const QString state = QStandardPaths::writableLocation(QStandardPaths::GenericStateLocation);
    if (!state.isEmpty())
        m_directory = state + QStringLiteral("/celestina/diagnostics");
    m_liveName = QStringLiteral("%1-%2.jsonl").arg(m_component, sanitized(runId()));
    m_startedMillis = static_cast<qint64>(nowMillis());

    m_writer = std::thread([this] { writerLoop(); });
}

void DiagnosticJournal::record(const Record &event)
{
    if (!m_open)
        return;

    Queued queued {
        event.m_level,
        event.m_event,
        event.m_fields,
        nowNanos(),
        nowMillis() - static_cast<quint64>(m_startedMillis),
        QString(),
    };

    {
        const std::lock_guard<std::mutex> guard(m_lock);
        if (static_cast<int>(m_waiting.size()) < maxQueue) {
            m_waiting.push_back(std::move(queued));
        } else if (!mustFlush(queued.level)) {
            // The explicit drop policy, identical to the crate's: what gives way
            // first is the ordinary event, and the loss is counted rather than
            // silent.
            ++m_dropped;
        } else {
            const auto ordinary = std::find_if(
                m_waiting.begin(),
                m_waiting.end(),
                [](const Queued &waiting) { return !mustFlush(waiting.level); }
            );
            if (ordinary != m_waiting.end()) {
                m_waiting.erase(ordinary);
                ++m_dropped;
                m_waiting.push_back(std::move(queued));
            } else {
                ++m_dropped;
            }
        }
    }
    m_signal.notify_one();
}

void DiagnosticJournal::writerLoop()
{
    while (!m_stopping.load()) {
        if (drainOnce() == false) {
            std::unique_lock<std::mutex> guard(m_lock);
            m_signal.wait_for(guard, std::chrono::milliseconds(200));
        }
    }

    // The deliberate exit: drain what is left, but only until the deadline. A
    // disk that has stopped answering must not hold the session's exit open.
    QElapsedTimer deadline;
    deadline.start();
    while (deadline.elapsed() < drainDeadlineMillis && drainOnce()) { }

    emitLine(Queued {Level::Critical, QStringLiteral("journal.stop"), {}, nowNanos(), 0, {}});
    if (m_file >= 0) {
        ::fsync(m_file);
        ::close(m_file);
        m_file = -1;
    }
    m_writerDone.store(true, std::memory_order_release);
}

bool DiagnosticJournal::drainOnce()
{
    quint64 dropped = 0;
    std::vector<Queued> batch;
    {
        const std::lock_guard<std::mutex> guard(m_lock);
        dropped = m_dropped;
        m_dropped = 0;
        const int take = std::min<int>(static_cast<int>(m_waiting.size()), 256);
        for (int index = 0; index < take; ++index) {
            batch.push_back(std::move(m_waiting.front()));
            m_waiting.pop_front();
        }
    }

    if (dropped > 0) {
        Record loss(Level::Warn, QStringLiteral("journal.dropped"));
        loss.unsigned_number(QStringLiteral("events"), dropped);
        emitLine(Queued {loss.m_level, loss.m_event, loss.m_fields, nowNanos(), 0, {}});
    }
    for (const Queued &queued : batch)
        emitLine(queued);
    return !batch.empty();
}

void DiagnosticJournal::emitLine(const Queued &queued)
{
    QByteArray line;
    line.append('{');
    line.append("\"v\":").append(QByteArray::number(schemaVersion));
    line.append(",\"t\":").append(QByteArray::number(queued.wallNanos));
    line.append(",\"mono_ms\":").append(QByteArray::number(queued.monotonicMillis));
    line.append(",\"level\":").append(jsonString(QString::fromLatin1(levelToken(queued.level))));
    line.append(",\"component\":").append(jsonString(m_component));
    line.append(",\"event\":").append(jsonString(queued.event));
    line.append(",\"run_id\":").append(jsonString(runId()));
    line.append(",\"pid\":")
        .append(QByteArray::number(static_cast<qint64>(QCoreApplication::applicationPid())));
    // The host is generation zero of a run: it is the process that named it.
    line.append(",\"generation\":0");
    if (!queued.worker.isEmpty())
        line.append(",\"worker\":").append(jsonString(queued.worker));
    line.append(queued.fields);
    line.append('}');

    if (line.size() > maxLineBytes) {
        // The event unusual enough to overflow is the one worth knowing about,
        // so it becomes a bounded record of the overflow rather than nothing.
        QByteArray overflow;
        overflow.append('{');
        overflow.append("\"v\":").append(QByteArray::number(schemaVersion));
        overflow.append(",\"t\":").append(QByteArray::number(queued.wallNanos));
        overflow.append(",\"level\":")
            .append(jsonString(QString::fromLatin1(levelToken(queued.level))));
        overflow.append(",\"component\":").append(jsonString(m_component));
        overflow.append(",\"event\":").append(jsonString(queued.event));
        overflow.append(",\"run_id\":").append(jsonString(runId()));
        overflow.append(",\"journal_overflow_bytes\":").append(QByteArray::number(line.size()));
        overflow.append('}');
        line = overflow;
    }

    if (m_mirror && mirrored(queued.level)) {
        qWarning().noquote() << QStringLiteral("celestina[%1/%2] %3 %4")
                                    .arg(m_component, runId())
                                    .arg(QString::fromLatin1(levelToken(queued.level)))
                                    .arg(queued.event);
    }

    if (!ensureFile(line.size() + 1))
        return;

    line.append('\n');
    const qint64 written = ::write(m_file, line.constData(), static_cast<size_t>(line.size()));
    if (written != line.size()) {
        ++m_writeFailures;
        ::close(m_file);
        m_file = -1;
        m_bytes = 0;
        m_unwritableSinceMillis = nowMillis();
        return;
    }
    m_bytes += written;
    if (mustFlush(queued.level))
        ::fdatasync(m_file);
}

bool DiagnosticJournal::ensureFile(int incoming)
{
    // The same arithmetic `diagnostics::rotation` performs.
    if (m_file >= 0 && m_bytes > 0 && m_bytes + incoming > maxFileBytes)
        rotate();
    if (m_file >= 0)
        return true;

    if (m_unwritableSinceMillis != 0
        && nowMillis() - m_unwritableSinceMillis < static_cast<quint64>(reopenAfterMillis))
        return false;
    if (m_directory.isEmpty()) {
        m_unwritableSinceMillis = nowMillis();
        return false;
    }
    if (!QDir().mkpath(m_directory)) {
        m_unwritableSinceMillis = nowMillis();
        return false;
    }
    // The journal names outputs, buses, processes and timings of one person's
    // session. Nobody else on this machine has business reading it.
    QFile::setPermissions(
        m_directory,
        QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner
    );

    const QByteArray path = (m_directory + QLatin1Char('/') + m_liveName).toUtf8();
    m_file = ::open(path.constData(), O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
    if (m_file < 0) {
        m_unwritableSinceMillis = nowMillis();
        return false;
    }

    m_bytes = QFileInfo(QString::fromUtf8(path)).size();
    // A previous run cut off mid-line leaves a file whose last byte is not a
    // newline. Closing that line keeps the torn record readable as one broken
    // line instead of fusing it with this run's first.
    if (m_bytes > 0) {
        QFile existing(QString::fromUtf8(path));
        if (existing.open(QIODevice::ReadOnly)) {
            existing.seek(existing.size() - 1);
            const QByteArray last = existing.read(1);
            existing.close();
            if (last != QByteArrayLiteral("\n"))
                m_bytes += ::write(m_file, "\n", 1);
        }
    }

    m_unwritableSinceMillis = 0;
    retireSurplus();

    if (m_writeFailures > 0) {
        const QByteArray recovered
            = QStringLiteral(
                  "{\"v\":%1,\"level\":\"warn\",\"component\":\"%2\",\"event\":\"journal.recovered\""
                  ",\"run_id\":\"%3\",\"failed_writes\":%4}\n"
              )
                  .arg(schemaVersion)
                  .arg(m_component, sanitized(runId()))
                  .arg(m_writeFailures)
                  .toUtf8();
        ::write(m_file, recovered.constData(), static_cast<size_t>(recovered.size()));
        m_writeFailures = 0;
    }
    return true;
}

void DiagnosticJournal::rotate()
{
    if (m_file < 0)
        return;
    ::fsync(m_file);
    ::close(m_file);
    m_file = -1;
    m_bytes = 0;

    const QString live = m_directory + QLatin1Char('/') + m_liveName;
    const QString retired = QStringLiteral("%1/%2-%3.%4.jsonl")
                                .arg(m_directory, m_component, sanitized(runId()))
                                .arg(nowNanos(), 0, 16);
    QFile::rename(live, retired);
}

void DiagnosticJournal::retireSurplus()
{
    QDir directory(m_directory);
    QStringList names = directory.entryList(
        {m_component + QStringLiteral("-*.jsonl")},
        QDir::Files,
        QDir::Name | QDir::Reversed
    );
    // Newest first. The live file has no timestamp segment, so it is put in
    // front explicitly rather than by luck.
    names.removeAll(m_liveName);
    names.prepend(m_liveName);
    for (int index = maxFiles; index < names.size(); ++index)
        QFile::remove(m_directory + QLatin1Char('/') + names.at(index));
}

void DiagnosticJournal::close()
{
    if (!m_open)
        return;
    m_stopping.store(true);
    m_signal.notify_all();
    const auto deadline = std::chrono::steady_clock::now()
        + std::chrono::milliseconds(drainDeadlineMillis);
    while (!m_writerDone.load(std::memory_order_acquire)
           && std::chrono::steady_clock::now() < deadline)
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    if (m_writer.joinable()) {
        if (m_writerDone.load(std::memory_order_acquire))
            m_writer.join();
        else
            m_writer.detach();
    }
    m_open = false;
}

DiagnosticJournal::~DiagnosticJournal()
{
    close();
}
