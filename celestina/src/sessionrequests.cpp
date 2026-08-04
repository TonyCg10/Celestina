#include "sessionrequests.h"

#include <utility>

namespace {
// The helper refuses a 33rd queued command, so tracking more here would
// promise bookkeeping it cannot honour.
constexpr qsizetype maxTrackedRequests = 32;
// Every tracked request can pass through pending and one terminal state; a
// caller that never drains loses the oldest transitions instead of the memory.
constexpr qsizetype maxRecordedOutcomes = 2 * maxTrackedRequests;

const QString &pendingState()
{
    static const QString state = QStringLiteral("pending");
    return state;
}

// A level asked for over D-Bus and a level published as JSON are the same
// number in two integer types. Comparing them as numbers keeps a confirmation
// from turning into a timeout over how each side happened to be typed.
bool sameReading(const QVariant &asked, const QVariant &reported)
{
    if (asked.userType() == QMetaType::Bool
        || reported.userType() == QMetaType::Bool) {
        return asked.userType() == reported.userType()
            && asked.toBool() == reported.toBool();
    }

    bool askedIsNumber = false;
    bool reportedIsNumber = false;
    const qlonglong left = asked.toLongLong(&askedIsNumber);
    const qlonglong right = reported.toLongLong(&reportedIsNumber);
    if (askedIsNumber && reportedIsNumber)
        return left == right;

    return asked == reported;
}
} // namespace

QVariant SessionRequests::observed(
    const QVariantMap &providers,
    const Expectation &expectation
)
{
    const QVariantMap values =
        providers.value(expectation.provider).toMap();
    // A named field is that field's value; an unnamed one makes the whole
    // provider the thing being watched for change.
    if (expectation.field.isEmpty())
        return QVariant(values);

    return values.value(expectation.field);
}

bool SessionRequests::arrived(
    const Entry &entry,
    const QVariantMap &providers
) const
{
    const QVariant now = observed(providers, entry.expectation);
    if (entry.expectation.field.isEmpty()) {
        // Nothing about this provider has moved, so nothing has been observed
        // to happen — including the case where it went away entirely.
        return now != entry.baseline;
    }

    // A field the provider does not carry is not a match: an absent reading is
    // unknown, not the value that was asked for.
    return now.isValid() && sameReading(entry.expectation.value, now);
}

void SessionRequests::finish(
    const Entry &entry,
    const QString &state,
    const QString &reason
)
{
    if (m_outcomes.size() >= maxRecordedOutcomes)
        m_outcomes.removeFirst();

    m_outcomes.append(Outcome {entry.requestId, entry.verb, state, reason});
}

bool SessionRequests::isFull() const
{
    return m_entries.size() >= maxTrackedRequests;
}

bool SessionRequests::begin(
    quint64 requestId,
    quint64 helperRequestId,
    const QString &verb,
    const Expectation &expectation,
    const QVariantMap &providers,
    qint64 nowMs
)
{
    if (isFull())
        return false;

    m_entries.append(Entry {
        requestId,
        helperRequestId,
        verb,
        expectation,
        observed(providers, expectation),
        nowMs + expectation.timeoutMs,
    });

    if (m_outcomes.size() >= maxRecordedOutcomes)
        m_outcomes.removeFirst();
    m_outcomes.append(Outcome {requestId, verb, pendingState(), QString()});
    return true;
}

void SessionRequests::acknowledge(
    quint64 helperRequestId,
    const QString &state,
    const QString &reason,
    qint64 nowMs
)
{
    Q_UNUSED(nowMs)

    for (qsizetype entry = 0; entry < m_entries.size(); ++entry) {
        if (m_entries.at(entry).helperRequestId != helperRequestId)
            continue;

        // Acceptance is not arrival: the request stays pending until the
        // provider says something, or until it runs out of time.
        if (state == QStringLiteral("accepted"))
            return;

        finish(
            m_entries.at(entry),
            QStringLiteral("failed"),
            reason.isEmpty()
                ? QStringLiteral("the provider helper refused the request")
                : reason
        );
        m_entries.removeAt(entry);
        return;
    }
}

void SessionRequests::applyProviders(
    const QVariantMap &providers,
    qint64 nowMs
)
{
    Q_UNUSED(nowMs)

    for (qsizetype entry = m_entries.size() - 1; entry >= 0; --entry) {
        if (!arrived(m_entries.at(entry), providers))
            continue;

        finish(m_entries.at(entry), QStringLiteral("confirmed"), QString());
        m_entries.removeAt(entry);
    }
}

void SessionRequests::expire(qint64 nowMs)
{
    for (qsizetype entry = m_entries.size() - 1; entry >= 0; --entry) {
        if (m_entries.at(entry).deadlineMs > nowMs)
            continue;

        finish(
            m_entries.at(entry),
            QStringLiteral("failed"),
            QStringLiteral(
                "the shell asked for this, but nothing reported it happening"
            )
        );
        m_entries.removeAt(entry);
    }
}

void SessionRequests::failAll(const QString &reason)
{
    for (const Entry &entry : std::as_const(m_entries))
        finish(entry, QStringLiteral("failed"), reason);

    m_entries.clear();
}

QList<SessionRequests::Outcome> SessionRequests::takeOutcomes()
{
    return std::exchange(m_outcomes, QList<Outcome>());
}
