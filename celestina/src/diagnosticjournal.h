#pragma once

#include <QByteArray>
#include <QString>

#include <atomic>
#include <condition_variable>
#include <cstdint>
#include <deque>
#include <mutex>
#include <thread>
#include <vector>

// The host's half of the structured diagnostic journal.
//
// # Why this exists in C++ at all
//
// The policy — the levels, the field names, the bounds, the redaction rule, the
// line format and the rotation arithmetic — is owned by
// `celestina-shell-core::diagnostics`, and both Rust helpers use it directly.
// This file is a declared *mirror* of that contract, not a second design, and
// the boundary it crosses is a real one:
//
//   * the host is a separate process, in a different language, that does not
//     link the crate — the helpers are spawned executables, not a library;
//   * the events the host most needs to record are exactly the ones that happen
//     when a helper is not there to record them: a helper failing to start, a
//     helper dying, the backoff between restarts, and the host's own shutdown;
//   * routing host events through a helper's pipe would lose precisely those.
//
// So each of the three processes writes its own file, all under one directory,
// all carrying the same `run_id`, and a reader merges them by timestamp. There
// is no shared writer, no cross-process lock and no file two processes append
// to — which is also what makes the files survive one process being killed.
//
// The one duty this file owes: the field names and level tokens here must stay
// identical to the crate's. A change on either side is a change to both.
//
// # What it may never do
//
// Block the Qt thread, grow without bound, throw, or stop the shell because a
// disk refused it. `record` returns void, cannot fail, and hands the event to a
// writer thread. Nothing a person wrote, received, played or opened is ever
// passed to it: sizes and technical identities only.
class DiagnosticJournal final
{
public:
    // Deliberately the same ladder the crate declares.
    enum class Level {
        Trace,
        Debug,
        Info,
        Warn,
        Error,
        // Flushed to the disk before returning. The failure this journal exists
        // for cuts power to the machine, so an event about a process, a helper's
        // death, a bus or shutdown must not be left in a buffer.
        Critical,
    };

    // One event under construction. Only numbers, flags, bounded technical
    // identities and sizes can be put in it — there is no method that takes a
    // window title, a notification body or a command line and keeps it.
    class Record final
    {
    public:
        Record(Level level, QString event);

        // A bounded technical identity: an output, a bus, a provider key, a
        // process name, a D-Bus name, an error reason.
        Record &text(const QString &key, const QString &value);
        Record &number(const QString &key, qint64 value);
        Record &unsigned_number(const QString &key, quint64 value);
        Record &millis(const QString &key, quint64 value);
        Record &flag(const QString &key, bool value);
        // Records that a value was present and how big it was, and forgets it.
        // This is the only way anything derived from a person's data may be
        // mentioned at all.
        Record &redacted(const QString &key, const QString &value);

    private:
        friend class DiagnosticJournal;

        Level m_level;
        QString m_event;
        QByteArray m_fields;
        int m_fieldCount = 0;
    };

    // The process-wide journal. Opening it twice is the same journal.
    static DiagnosticJournal &instance();

    // The identifier every process of this invocation shares. Generated once by
    // the host and handed to both helpers in their environment before they
    // start.
    static QString runId();

    // Puts `CELESTINA_RUN_ID` into a helper's environment, so its lines land in
    // the same run as the host's.
    static void exportRunId();

    // Starts the writer and records the opening line. Safe to call once.
    void open(const QString &component);

    // Takes the record by reference because the builder chain returns one, and
    // because copying a bounded field buffer is cheaper than the write it feeds.
    void record(const Record &event);

    // Drains within a bounded time and joins the writer, so no writer outlives
    // the host. Calling it twice is harmless.
    void close();

    ~DiagnosticJournal();

    DiagnosticJournal(const DiagnosticJournal &) = delete;
    DiagnosticJournal &operator=(const DiagnosticJournal &) = delete;

private:
    DiagnosticJournal() = default;

    struct Queued {
        Level level;
        QString event;
        QByteArray fields;
        quint64 wallNanos;
        quint64 monotonicMillis;
        QString worker;
    };

    void writerLoop();
    bool drainOnce();
    void emitLine(const Queued &queued);
    bool ensureFile(int incoming);
    void rotate();
    void retireSurplus();

    QString m_component;
    QString m_directory;
    QString m_liveName;
    int m_file = -1;
    qint64 m_bytes = 0;
    qint64 m_startedMillis = 0;
    bool m_mirror = true;
    quint64 m_unwritableSinceMillis = 0;
    quint64 m_writeFailures = 0;

    std::deque<Queued> m_waiting;
    quint64 m_dropped = 0;
    std::mutex m_lock;
    std::condition_variable m_signal;
    std::atomic<bool> m_stopping {false};
    std::atomic<bool> m_writerDone {false};
    std::thread m_writer;
    bool m_open = false;
};

// Short names for the call sites, which are many and would otherwise be noise.
#define CELESTINA_JOURNAL(level, event) \
    DiagnosticJournal::Record(DiagnosticJournal::Level::level, QStringLiteral(event))
