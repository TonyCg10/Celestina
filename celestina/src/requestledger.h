#pragma once

#include <QList>
#include <QObject>
#include <QString>
#include <QVariantList>
#include <QVariantMap>

// What the ledger needs from the bridge, and nothing else: a way to send a
// request and learn the id it will be answered under.
//
// Narrow on purpose. It is what lets the ledger's own contract — two policies,
// supersession, generation loss, bounds — be driven without a helper process,
// and it keeps the ledger from reaching into process lifecycle it does not own.
class RequestSink
{
public:
    virtual ~RequestSink() = default;

    // Zero means the request could not be sent at all.
    virtual quint64 sendRequest(
        const QString &provider,
        const QString &verb,
        const QVariantMap &options
    ) = 0;
};

// What became of the requests a surface made, kept where a surface cannot take
// it with it.
//
// A menu row is a `MenuItem`, and activating one closes its `Menu`. The surface
// is dismissed, the window is destroyed, and anything the window owned goes with
// it — before the helper has even written `accepted`. A ledger that lived in the
// menu therefore reported nothing, and reopening the menu produced an empty one.
// So it lives here, on the provider bridge, whose lifetime is already exactly
// what a request's lifetime should be: one helper generation.
//
// Two contracts share it, declared per request rather than guessed from a verb:
//
//   * `Immediate` — the helper answering `accepted` is the whole answer. Every
//     verb the control centre had before connectivity existed works this way;
//     nothing ever sends them a `confirmed`, so waiting for one would leave a
//     control saying "asking" for ever.
//   * `Confirmed` — `accepted` means a tool ran, and the request keeps waiting
//     until a later observation of the machine says `confirmed` or `failed`.
//     This is what `UX-1-B` gives the connectivity verbs, and the only reason a
//     menu may show a state nothing has observed is that it does not.
//
// Request ids are `quint64` on the wire and stay `quint64` here. They cross to
// QML only as decimal strings, because a JavaScript number cannot hold one
// without losing the low bits.
class RequestLedger final : public QObject
{
    Q_OBJECT
    // Bumped whenever anything here changes, so a QML binding that reads it has
    // a dependency that really moves. The same idiom `ProviderStates` uses.
    Q_PROPERTY(qulonglong revision READ revision NOTIFY changed)

public:
    // The two contracts, as they are named on the QML boundary.
    static const QString ImmediatePolicy;
    static const QString ConfirmedPolicy;

    // What a request is doing. Tokens, not copy: the words a person reads are
    // the surface's business and are Spanish there.
    static const QString PendingState;
    static const QString ConfirmedState;
    static const QString FailedState;

    // Why a failed request failed, for a surface to turn into its own sentence.
    static const QString UnsentCause;
    static const QString ReportedCause;
    static const QString GenerationLostCause;

    // Past this many tracked targets the oldest is dropped. A person clicking
    // menu rows produces a handful; an unbounded ledger would keep every one of
    // a session's requests alive for the life of the helper.
    static constexpr int maxEntries = 64;

    explicit RequestLedger(RequestSink *sink, QObject *parent = nullptr);

    qulonglong revision() const { return m_revision; }

    // Sends a verb and starts tracking it under `provider` and `target`.
    //
    // Returns the request id as a decimal string, or an empty string when the
    // request could not be sent — which is a failure now, recorded as one,
    // rather than a wait that will never end. A newer request for the same
    // target replaces the older one, whose late result then answers nothing.
    Q_INVOKABLE QString send(
        const QString &provider,
        const QString &verb,
        const QVariantMap &options,
        const QString &target,
        const QString &policy
    );

    // `{"state": ..., "cause": ...}`, or an empty map for a target nothing is
    // known about. `cause` is empty unless the state is `failed`.
    Q_INVOKABLE QVariantMap stateOf(const QString &provider, const QString &target) const;
    Q_INVOKABLE bool isPending(const QString &provider, const QString &target) const;

    // Every failed target of one provider, oldest first, as
    // `{"target": ..., "cause": ...}`. What a menu shows for a row that has
    // gone away since: the failure stays visible instead of vanishing with it.
    Q_INVOKABLE QVariantList failures(const QString &provider) const;

    // Forgets one target. A surface calls this when a person has seen a failure
    // and acts again, so a stale report cannot outlive its usefulness.
    Q_INVOKABLE void forget(const QString &provider, const QString &target);

    // The helper answered. Called by the bridge with the id it really carries,
    // so no id ever passes through a double.
    void result(qulonglong requestId, const QString &state, const QString &reason);

    // The helper this ledger belongs to is gone. Nothing it accepted will ever
    // be answered, and a replacement has run none of it.
    void generationLost();

signals:
    void changed();

private:
    struct Entry {
        QString provider;
        QString target;
        quint64 requestId = 0;
        bool confirmedContract = false;
        QString state;
        QString cause;
    };

    int indexOf(const QString &provider, const QString &target) const;
    int indexOfRequest(quint64 requestId) const;
    void settle(Entry &entry, const QString &state, const QString &cause);

    RequestSink *m_sink;
    QList<Entry> m_entries;
    qulonglong m_revision = 0;
};
