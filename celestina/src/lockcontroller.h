#pragma once

#include <QDBusUnixFileDescriptor>
#include <QObject>
#include <QPointer>
#include <QProcess>
#include <QString>

#include <functional>

// The shell's half of the session lock: it starts the lock, knows when the
// compositor has confirmed it, and refuses to let anything sleep an unlocked
// session.
//
// ADR 0004 puts two rules here. Suspend happens only behind a confirmed lock,
// and a lock that does not come up is a refusal rather than a suspend — this
// class never resolves an uncertainty in favour of sleeping. The lock itself
// is `celestina-lock`, a separate process, so what this owns is its lifetime
// and the sequencing around it, never the covering or the authentication.
//
// System-initiated sleep — a lid, an idle timer, `systemctl suspend` from
// anywhere — is caught with a logind delay inhibitor. logind announces
// `PrepareForSleep(true)` and waits for every delay inhibitor to be released;
// this one is released when the lock is confirmed, so the machine goes to
// sleep behind a locked screen instead of racing it.
class LockController final : public QObject
{
    Q_OBJECT

public:
    explicit LockController(QObject *parent = nullptr);
    ~LockController() override;

    // Whether a lock process is running and the compositor has confirmed it.
    // "Running but unconfirmed" is deliberately not locked: the screen may not
    // be covered yet.
    bool isLocked() const { return m_confirmed; }
    bool isStarting() const;

    // Starts the lock if it is not already up. Returns false when the lock
    // could not even be started — no binary, or it refused — which callers
    // must treat as "the session is not locked".
    bool lock();

    // Locks, waits for the compositor to confirm, and only then suspends.
    // `answer` receives an empty string on success or the reason it was
    // refused; it is called exactly once. Nothing here suspends on a timeout.
    void lockAndSuspend(std::function<void(const QString &)> answer);

signals:
    void lockedChanged();

private slots:
    // Connected to logind by name, so it has to be a slot.
    void prepareForSleep(bool starting);

private:
    void started(const QString &line);
    void finished();
    void takeSleepInhibitor();
    void releaseSleepInhibitor();
    void suspendNow(std::function<void(const QString &)> answer);

    QProcess *m_process;
    bool m_confirmed = false;
    // Held while this session may be asked to sleep, and dropped only once a
    // lock is confirmed. An invalid descriptor means logind is free to sleep
    // whenever it likes — which is correct once the screen is covered.
    QDBusUnixFileDescriptor m_sleepInhibitor;
    // Set while a `PrepareForSleep` is being answered, so a lock confirmed for
    // that reason releases the inhibitor instead of holding the machine awake.
    bool m_sleepPending = false;
};
