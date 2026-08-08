#include "requestledger.h"

#include <QDebug>

const QString RequestLedger::ImmediatePolicy = QStringLiteral("immediate");
const QString RequestLedger::ConfirmedPolicy = QStringLiteral("confirmed");

const QString RequestLedger::PendingState = QStringLiteral("pending");
const QString RequestLedger::ConfirmedState = QStringLiteral("confirmed");
const QString RequestLedger::FailedState = QStringLiteral("failed");

const QString RequestLedger::UnsentCause = QStringLiteral("unsent");
const QString RequestLedger::ReportedCause = QStringLiteral("reported");
const QString RequestLedger::GenerationLostCause = QStringLiteral("generation-lost");

RequestLedger::RequestLedger(RequestSink *sink, QObject *parent)
    : QObject(parent)
    , m_sink(sink)
{
}

int RequestLedger::indexOf(const QString &provider, const QString &target) const
{
    for (int index = 0; index < m_entries.size(); ++index) {
        if (m_entries.at(index).provider == provider && m_entries.at(index).target == target)
            return index;
    }
    return -1;
}

int RequestLedger::indexOfRequest(quint64 requestId) const
{
    // Zero is never a real id — the bridge answers it for a request it could
    // not send — so it must not match an entry that is no longer waiting.
    if (requestId == 0)
        return -1;

    for (int index = 0; index < m_entries.size(); ++index) {
        if (m_entries.at(index).requestId == requestId)
            return index;
    }
    return -1;
}

void RequestLedger::settle(Entry &entry, const QString &state, const QString &cause)
{
    entry.state = state;
    entry.cause = cause;
    // A settled entry answers no further result. Without this a late frame
    // carrying the same id would reopen a request that is already reported.
    entry.requestId = 0;
}

QString RequestLedger::send(
    const QString &provider,
    const QString &verb,
    const QVariantMap &options,
    const QString &target,
    const QString &policy
)
{
    const bool confirmedContract = (policy == ConfirmedPolicy);
    if (!confirmedContract && policy != ImmediatePolicy) {
        qWarning() << "Celestina refused a request with no known contract:" << policy;
        return QString();
    }

    int at = indexOf(provider, target);
    if (at < 0) {
        // Bounded. The oldest tracked target goes rather than the ledger
        // growing for the life of the helper.
        if (m_entries.size() >= maxEntries)
            m_entries.removeFirst();

        m_entries.append(Entry {provider, target, 0, confirmedContract, QString(), QString()});
        at = m_entries.size() - 1;
    }

    Entry &entry = m_entries[at];
    entry.confirmedContract = confirmedContract;
    // Whatever this target was waiting for is replaced here: the old id is
    // dropped, so its answer arrives to nothing rather than settling the newer
    // request in its place.
    entry.requestId = 0;

    const quint64 id = m_sink ? m_sink->sendRequest(provider, verb, options) : 0;
    if (id == 0) {
        settle(entry, FailedState, UnsentCause);
    } else {
        entry.requestId = id;
        entry.state = PendingState;
        entry.cause.clear();
    }

    ++m_revision;
    emit changed();
    // Decimal, never a number: a `quint64` past 2^53 cannot survive a
    // JavaScript double, and this is the value that identifies the request.
    return id == 0 ? QString() : QString::number(id);
}

void RequestLedger::result(qulonglong requestId, const QString &state, const QString &reason)
{
    const int at = indexOfRequest(requestId);
    // Not ours, already settled, or superseded by a newer request for the same
    // target. Either way it answers nothing that is still on screen.
    if (at < 0)
        return;

    Entry &entry = m_entries[at];
    if (state == QStringLiteral("failed")) {
        // The helper's own reason is English by contract. It is a diagnostic:
        // logged here, and never carried to a surface that speaks Spanish.
        if (!reason.isEmpty())
            qWarning().noquote() << "Celestina's provider request failed:" << reason;

        settle(entry, FailedState, ReportedCause);
    } else if (state == QStringLiteral("confirmed")) {
        settle(entry, ConfirmedState, QString());
    } else if (state == QStringLiteral("accepted")) {
        // The whole answer for an immediate verb; only the start of one for a
        // request whose effect something still has to observe.
        if (!entry.confirmedContract)
            settle(entry, ConfirmedState, QString());
        else
            entry.state = PendingState;
    } else {
        // A state this shell does not know. Reported rather than guessed at.
        qWarning() << "Celestina received an unknown request state:" << state;
        return;
    }

    ++m_revision;
    emit changed();
}

void RequestLedger::generationLost()
{
    bool changedAnything = false;
    for (Entry &entry : m_entries) {
        if (entry.state != PendingState)
            continue;

        settle(entry, FailedState, GenerationLostCause);
        changedAnything = true;
    }
    if (!changedAnything)
        return;

    ++m_revision;
    emit changed();
}

QVariantMap RequestLedger::stateOf(const QString &provider, const QString &target) const
{
    const int at = indexOf(provider, target);
    if (at < 0 || m_entries.at(at).state.isEmpty())
        return QVariantMap();

    return QVariantMap {
        {QStringLiteral("state"), m_entries.at(at).state},
        {QStringLiteral("cause"), m_entries.at(at).cause},
    };
}

bool RequestLedger::isPending(const QString &provider, const QString &target) const
{
    const int at = indexOf(provider, target);
    return at >= 0 && m_entries.at(at).state == PendingState;
}

QVariantList RequestLedger::failures(const QString &provider) const
{
    QVariantList found;
    for (const Entry &entry : m_entries) {
        if (entry.provider != provider || entry.state != FailedState)
            continue;

        found.append(QVariantMap {
            {QStringLiteral("target"), entry.target},
            {QStringLiteral("cause"), entry.cause},
        });
    }
    return found;
}

void RequestLedger::forget(const QString &provider, const QString &target)
{
    const int at = indexOf(provider, target);
    if (at < 0)
        return;

    m_entries.removeAt(at);
    ++m_revision;
    emit changed();
}
