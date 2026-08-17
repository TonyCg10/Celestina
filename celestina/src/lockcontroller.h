#pragma once

#include <QDBusUnixFileDescriptor>
#include <QObject>
#include <QPointer>
#include <QProcess>
#include <QString>
#include <QVariantMap>

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

    // Which image the session is showing on each output, keyed by output name,
    // as the wallpaper provider last published it. The lock is handed this at
    // the moment it starts so a covered screen can show the picture it was
    // already showing instead of a bare canvas.
    //
    // It is deliberately a plain map rather than a provider connection: which
    // file belongs to which output is the shell's decision and stays here. The
    // lock is told the answer and never asks the question, which is also why
    // this class needs no provider type to be tested.
    void setBackdrop(const QVariantMap &wallpapersByOutput);

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
    // Writes the backdrop line, if there is one, and closes the lock's stdin.
    // Never waits for it: the bytes go into the pipe buffer and the lock reads
    // them when it gets to them. Covering the screen does not wait for this and
    // must not — see `sendBackdrop`'s own note.
    void sendBackdrop();
    void takeSleepInhibitor();
    void releaseSleepInhibitor();
    void suspendNow(std::function<void(const QString &)> answer);

    QProcess *m_process;
    // The wallpaper choice as of the last provider frame. Read only when a
    // lock starts, so a change while the screen is covered does not reach a
    // surface that is already up.
    QVariantMap m_backdrop;
    bool m_confirmed = false;
    // Held while this session may be asked to sleep, and dropped only once a
    // lock is confirmed. An invalid descriptor means logind is free to sleep
    // whenever it likes — which is correct once the screen is covered.
    QDBusUnixFileDescriptor m_sleepInhibitor;
    // Set while a `PrepareForSleep` is being answered, so a lock confirmed for
    // that reason releases the inhibitor instead of holding the machine awake.
    bool m_sleepPending = false;
};
