#include "workspacefocusrequests.h"

#include <utility>

namespace {
// The adapter refuses a 33rd queued command, so tracking more here would
// promise bookkeeping the helper cannot honour.
constexpr qsizetype maxTrackedRequests = 32;
// Every tracked request can pass through pending and one terminal state; a
// caller that never drains loses the oldest transitions instead of the memory.
constexpr qsizetype maxRecordedOutcomes = 2 * maxTrackedRequests;
} // namespace

WorkspaceFocusRequests::WorkspaceFocusRequests(Timings timings)
    : m_timings(timings)
{
}

qsizetype WorkspaceFocusRequests::indexOfTarget(
    const QString &output,
    int index
) const
{
    for (qsizetype entry = 0; entry < m_entries.size(); ++entry) {
        if (m_entries.at(entry).output == output
            && m_entries.at(entry).index == index) {
            return entry;
        }
    }
    return -1;
}

qsizetype WorkspaceFocusRequests::indexOfRequest(quint64 requestId) const
{
    for (qsizetype entry = 0; entry < m_entries.size(); ++entry) {
        if (m_entries.at(entry).requestId == requestId)
            return entry;
    }
    return -1;
}

void WorkspaceFocusRequests::record(quint64 requestId, const QString &state)
{
    if (m_outcomes.size() >= maxRecordedOutcomes)
        m_outcomes.removeFirst();

    m_outcomes.append(Outcome {requestId, state});
}

void WorkspaceFocusRequests::enterTerminal(
    Entry &entry,
    State state,
    qint64 nowMs
)
{
    entry.state = state;
    entry.deadlineMs = nowMs
        + (state == State::Confirmed ? m_timings.confirmedHoldMs
                                     : m_timings.failedHoldMs);
    record(
        entry.requestId,
        state == State::Confirmed ? QStringLiteral("confirmed")
                                  : QStringLiteral("failed")
    );
}

QList<WorkspaceFocusRequests::Outcome> WorkspaceFocusRequests::takeOutcomes()
{
    return std::exchange(m_outcomes, QList<Outcome>());
}

bool WorkspaceFocusRequests::begin(
    quint64 requestId,
    quint64 generation,
    const QString &output,
    int index,
    qint64 nowMs
)
{
    const qsizetype existing = indexOfTarget(output, index);
    if (existing >= 0 && m_entries.at(existing).state == State::Pending)
        return false;
    if (existing < 0 && m_entries.size() >= maxTrackedRequests)
        return false;

    const Entry entry {
        requestId,
        generation,
        output,
        index,
        State::Pending,
        nowMs + m_timings.pendingTimeoutMs,
    };
    // A held confirmation or failure for the same workspace is replaced: the
    // newest request is the one the panel must report on.
    if (existing >= 0)
        m_entries[existing] = entry;
    else
        m_entries.append(entry);

    record(requestId, QStringLiteral("pending"));
    return true;
}

bool WorkspaceFocusRequests::acknowledge(
    quint64 requestId,
    quint64 generation,
    bool accepted,
    qint64 nowMs
)
{
    const qsizetype position = indexOfRequest(requestId);
    if (position < 0)
        return false;

    Entry &entry = m_entries[position];
    // A result from a previous adapter process says nothing about this one.
    if (entry.generation != generation || entry.state != State::Pending)
        return false;

    if (accepted) {
        // Acceptance is not arrival: the request stays pending until a
        // snapshot shows the requested workspace active.
        return false;
    }

    enterTerminal(entry, State::Failed, nowMs);
    return true;
}

bool WorkspaceFocusRequests::applyActive(
    const QHash<QString, int> &activeByOutput,
    qint64 nowMs
)
{
    bool changed = false;
    for (Entry &entry : m_entries) {
        if (entry.state != State::Pending)
            continue;

        const auto active = activeByOutput.constFind(entry.output);
        if (active == activeByOutput.constEnd() || active.value() != entry.index)
            continue;

        enterTerminal(entry, State::Confirmed, nowMs);
        changed = true;
    }
    return changed;
}

bool WorkspaceFocusRequests::expire(qint64 nowMs)
{
    bool changed = false;
    for (qsizetype entry = m_entries.size() - 1; entry >= 0; --entry) {
        if (m_entries.at(entry).deadlineMs > nowMs)
            continue;

        if (m_entries.at(entry).state == State::Pending) {
            enterTerminal(m_entries[entry], State::Failed, nowMs);
            changed = true;
            continue;
        }

        m_entries.removeAt(entry);
        changed = true;
    }
    return changed;
}

bool WorkspaceFocusRequests::failAll(qint64 nowMs)
{
    bool changed = false;
    for (Entry &entry : m_entries) {
        if (entry.state != State::Pending)
            continue;

        enterTerminal(entry, State::Failed, nowMs);
        changed = true;
    }
    return changed;
}

QString WorkspaceFocusRequests::stateFor(const QString &output, int index) const
{
    const qsizetype position = indexOfTarget(output, index);
    if (position < 0)
        return QString();

    switch (m_entries.at(position).state) {
    case State::Pending:
        return QStringLiteral("pending");
    case State::Confirmed:
        return QStringLiteral("confirmed");
    case State::Failed:
        return QStringLiteral("failed");
    }
    return QString();
}
