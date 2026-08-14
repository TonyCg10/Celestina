#pragma once

#include <QObject>
#include <QProcess>
#include <QString>

// Asks whether a passphrase unlocks this session, without ever answering it.
//
// ADR 0004: Celestina owns the lock surface and owns no password verification.
// This class is the shell's half of that boundary — it spawns
// `celestina-lock-verify`, hands it the passphrase down a pipe, and reports
// the verdict that child exited with. It runs no PAM conversation, reads no
// credential file, and has no branch that decides an attempt succeeded on its
// own.
//
// Everything that is not an explicit authentication is a refusal. There is no
// timeout that gives up into success, no missing-helper path that assumes the
// best, and no verdict inferred from anything but the child's exit status.
class LockAuthenticator final : public QObject
{
    Q_OBJECT

public:
    // What the child answered, and the only three answers there are. Only
    // `Authenticated` may unlock; `Refused` and `Unavailable` differ solely in
    // what the surface tells the person, never in what the lock does.
    enum class Verdict {
        Authenticated,
        Refused,
        Unavailable,
    };
    Q_ENUM(Verdict)

    explicit LockAuthenticator(QObject *parent = nullptr);

    // The user whose passphrase unlocks this session. Defaults to the one the
    // shell runs as, which is the only account a session lock may accept.
    QString user() const { return m_user; }
    void setUser(const QString &user) { m_user = user; }

    // The PAM service whose stack decides. A packaged Celestina installs
    // `celestina-lock`; the default falls back to the stack every distribution
    // already has.
    void setService(const QString &service) { m_service = service; }

    bool isBusy() const;

    // Starts one attempt. `secret` is written to the child's stdin and wiped
    // from this process before the call returns; it is never an argument, an
    // environment entry, a property or a log line. A second call while one is
    // in flight is refused rather than queued: an attempt the person did not
    // watch begin is not an attempt they made.
    void authenticate(QString secret);

    // Abandons an attempt in flight — the surface going away, the session
    // ending. No verdict is emitted for it.
    void cancel();

signals:
    // Exactly one of these per accepted `authenticate`, unless it was
    // cancelled. A verdict and nothing else: what the person is told about it
    // is the lock surface's wording, in QML where this shell keeps every
    // string it shows. Nothing here describes what the child was given.
    void answered(LockAuthenticator::Verdict verdict);

private:
    void finished(int exitCode, QProcess::ExitStatus status);

    QProcess *m_process;
    QString m_user;
    QString m_service;
};
