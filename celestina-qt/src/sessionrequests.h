#pragma once

#include <QList>
#include <QString>
#include <QVariantMap>

// Bookkeeping for the session verbs the shell forwards to the provider helper.
//
// A key binding is a request. The helper answering `accepted` means it carried
// the request out, never that the device changed: what a session verb did shows
// up in that provider's next value, or it did not happen. This class owns that
// distinction — which provider a request is waiting on, what would count as
// having arrived, and how long the shell may claim to be waiting.
//
// It is deliberately pure policy: time arrives as a monotonic millisecond
// stamp instead of a timer and provider state arrives as the map the helper
// published, so every transition is testable without an event loop or a
// process. It knows nothing about the session verb vocabulary, which
// `celestina-shell-core` owns; it only knows what an observation looks like.
class SessionRequests
{
public:
    // One state transition, in the order it happened. The caller drains these
    // and reports each on the session bus.
    struct Outcome {
        quint64 requestId = 0;
        QString verb;
        QString state;
        QString reason;
    };

    // What would count as this request having arrived.
    //
    // A verb that named an absolute value — `volume-set level=40` — is
    // confirmed only by that value being reported. A verb whose result cannot
    // be predicted from the request alone — a step, a toggle — is confirmed by
    // its provider publishing anything different from what it was showing when
    // the request was made. Both are observations; neither is the helper's
    // acknowledgement.
    struct Expectation {
        QString provider;
        // Empty for a request whose target the shell cannot name in advance.
        QString field;
        QVariant value;
        // How long the device is allowed to take. DDC is seconds, wpctl is
        // immediate, so the caller passes what its provider deserves.
        qint64 timeoutMs = 3000;
    };

    SessionRequests() = default;

    // Refuses a table that is already full; the caller must not send a command
    // when this returns false. `providers` is what the panel is showing now,
    // which is the baseline an unpredictable result is compared against.
    bool begin(
        quint64 requestId,
        quint64 helperRequestId,
        const QString &verb,
        const Expectation &expectation,
        const QVariantMap &providers,
        qint64 nowMs
    );
    // The helper's own answer. `accepted` leaves the request pending on
    // purpose; anything else ends it with the helper's reason.
    void acknowledge(
        quint64 helperRequestId,
        const QString &state,
        const QString &reason,
        qint64 nowMs
    );
    // A newly published set of provider values.
    void applyProviders(const QVariantMap &providers, qint64 nowMs);
    // Ends every request that has waited past its own timeout.
    void expire(qint64 nowMs);
    // The helper died, restarted or went unavailable: nothing in flight can
    // still be observed.
    void failAll(const QString &reason);

    // Returns the transitions recorded since the last call and forgets them.
    QList<Outcome> takeOutcomes();

    bool isEmpty() const { return m_entries.isEmpty(); }
    // The caller checks this before sending, so a request the table cannot
    // track is never carried out untracked.
    bool isFull() const;

private:
    struct Entry {
        quint64 requestId = 0;
        quint64 helperRequestId = 0;
        QString verb;
        Expectation expectation;
        QVariant baseline;
        qint64 deadlineMs = 0;
    };

    static QVariant observed(
        const QVariantMap &providers,
        const Expectation &expectation
    );
    bool arrived(const Entry &entry, const QVariantMap &providers) const;
    void finish(const Entry &entry, const QString &state, const QString &reason);

    QList<Entry> m_entries;
    QList<Outcome> m_outcomes;
};
