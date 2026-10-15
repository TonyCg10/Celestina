#pragma once

#include <QObject>

#include <functional>

// The lock, as everything above the protocol needs to see it.
//
// The Wayland object lives in the shell-integration plugin, behind Qt's
// private client API; this is the only thing the application, its QML and its
// tests ever touch. Keeping the seam here means the program that decides when
// to unlock never includes a header that changes between Qt patch releases,
// and can be reasoned about without the protocol in view.
//
// There is exactly one of these per process, created by the plugin when the
// compositor grants the lock.
class LockSession final : public QObject
{
    Q_OBJECT

public:
    // Null until the compositor has granted a lock. A null session means
    // nothing is covering the screen — never that it is safe to proceed.
    static LockSession *instance();

    // True once the compositor confirmed the session is locked. Nothing may
    // be sequenced behind the lock before this.
    bool isConfirmed() const { return m_confirmed; }

    // Releases the session. ADR 0004 allows exactly one caller: an
    // authenticated verdict. Ignored unless the lock was confirmed and has not
    // already been released — unlocking a lock that never took is a protocol
    // error the compositor may kill this client for, and a client killed after
    // destroying its lock is an exposed session.
    void release();

signals:
    void confirmed();
    // The compositor refused the lock or ended it. The session is not ours to
    // unlock; the only correct response is to leave without touching it.
    void finished();

private:
    friend class SessionLock;
    explicit LockSession(QObject *parent = nullptr);

    void markConfirmed();
    void markFinished();

    bool m_confirmed = false;
    bool m_released = false;
    // Set by the plugin so `release()` can reach the protocol object without
    // this header knowing what one is.
    std::function<void()> m_unlock;
};
