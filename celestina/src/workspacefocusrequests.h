#pragma once

#include <QHash>
#include <QList>
#include <QString>

// Bookkeeping for the panel's "focus this workspace" requests.
//
// A click is a request. The panel may only claim success once a later
// compositor snapshot reports the requested workspace active on the requested
// output; the adapter's acknowledgement means Niri accepted the request, never
// that it took effect. This class owns that distinction, the bounded request
// table and the timeout that turns a silent request into a visible failure.
//
// It is deliberately pure policy: time arrives as a monotonic millisecond
// stamp instead of a timer, so every transition is testable without an event
// loop, and no Qt object lifetime is involved. Every mutator returns whether
// the observable state changed, so the caller emits one signal per change.
class WorkspaceFocusRequests
{
public:
    // One state transition, in the order it happened. The caller drains these
    // to report a request's life to whoever asked for it — the panel reads
    // `stateFor`, the session bus reports each transition.
    struct Outcome {
        quint64 requestId = 0;
        QString state;
    };

    struct Timings {
        // Niri answers a focus action in milliseconds. Anything past this is
        // either lost or ignored, and the panel must stop claiming to be busy.
        qint64 pendingTimeoutMs = 2000;
        qint64 confirmedHoldMs = 900;
        qint64 failedHoldMs = 2500;
    };

    WorkspaceFocusRequests() = default;
    explicit WorkspaceFocusRequests(Timings timings);

    // Refuses a target that already has a request in flight and a table that
    // is full; the caller must not send a command when this returns false.
    bool begin(
        quint64 requestId,
        quint64 generation,
        const QString &output,
        int index,
        qint64 nowMs
    );
    // An accepted request stays pending: acceptance is not arrival.
    bool acknowledge(
        quint64 requestId,
        quint64 generation,
        bool accepted,
        qint64 nowMs
    );
    // `activeByOutput` maps an output name to the workspace index the
    // compositor currently reports as active there.
    bool applyActive(const QHash<QString, int> &activeByOutput, qint64 nowMs);
    // Times out pending requests and drops terminal states once their hold has
    // elapsed, so the panel returns to plain compositor truth.
    bool expire(qint64 nowMs);
    // The adapter died, restarted or went unavailable: nothing in flight can
    // still be confirmed.
    bool failAll(qint64 nowMs);

    // Returns the transitions recorded since the last call and forgets them.
    QList<Outcome> takeOutcomes();

    bool isEmpty() const { return m_entries.isEmpty(); }
    // "pending", "confirmed", "failed", or an empty string when this workspace
    // carries no request state.
    QString stateFor(const QString &output, int index) const;

private:
    enum class State {
        Pending,
        Confirmed,
        Failed,
    };

    struct Entry {
        quint64 requestId = 0;
        quint64 generation = 0;
        QString output;
        int index = 0;
        State state = State::Pending;
        qint64 deadlineMs = 0;
    };

    qsizetype indexOfTarget(const QString &output, int index) const;
    qsizetype indexOfRequest(quint64 requestId) const;
    void enterTerminal(Entry &entry, State state, qint64 nowMs);
    void record(quint64 requestId, const QString &state);

    Timings m_timings;
    QList<Entry> m_entries;
    // Bounded like the table itself: an undrained log is a caller bug, not a
    // reason to grow without limit.
    QList<Outcome> m_outcomes;
};
