#pragma once

#include <QObject>
#include <QProcess>
#include <QString>
#include <QtQml/qqmlregistration.h>

// Runs one polkit authorization conversation, without ever deciding it.
//
// ADR 0005: Celestina owns the prompt and owns no verification. This class is
// the shell's half of that boundary — it spawns `celestina-polkit-converse`,
// relays what PAM asks to whatever surface is prompting, hands the person's
// answer back down the pipe, and reports the verdict the child exited with. It
// runs no PAM conversation, speaks to no helper itself, and has no branch that
// decides an attempt succeeded on its own.
//
// Everything that is not an explicit authentication is a denial. There is no
// timeout that gives up into success, no missing-helper path that assumes the
// best, and no verdict inferred from anything but the child's exit status.
class PolkitConversation : public QObject
{
    Q_OBJECT
    // Named in QML so a prompt can read verdicts by name rather than by
    // number, and uncreatable there because the agent owns its conversations.
    // Not `final`: the type registration Qt generates derives from it.
    QML_ELEMENT
    QML_UNCREATABLE("The polkit agent gives a prompt its conversation.")
    Q_PROPERTY(bool busy READ isBusy NOTIFY busyChanged)

public:
    // What the child answered, and the only three answers there are. Only
    // `Authenticated` means polkitd granted the action; `Refused` and
    // `Unavailable` differ solely in what the prompt says, never in what the
    // shell does with them.
    enum class Verdict {
        Authenticated,
        Refused,
        Unavailable,
    };
    Q_ENUM(Verdict)

    explicit PolkitConversation(QObject *parent = nullptr);

    bool isBusy() const;

    // Begins one conversation for `user` against polkitd's `cookie`. The
    // cookie goes down the pipe, never on a command line: /proc shows every
    // process's arguments to every other one.
    void start(const QString &user, QString cookie);

    // The person's answer to the last request. Wiped from this process before
    // the call returns; it is never an argument, an environment entry, a
    // property, a journal line or a signal payload.
    Q_INVOKABLE void respond(QString secret);

    // Abandons the conversation — the prompt dismissed, polkitd cancelling,
    // the session ending. No verdict is emitted for it.
    Q_INVOKABLE void cancel();

signals:
    void busyChanged();

    // What PAM asked, relayed exactly as it was given. The shell writes none
    // of these strings and translates none of them: a prompt that paraphrased
    // what the stack asked would be telling the person something no component
    // here is entitled to decide.
    void secretRequested(const QString &prompt);
    void visibleRequested(const QString &prompt);
    void informed(const QString &text);
    void problemReported(const QString &text);

    // Exactly one of these per started conversation, unless it was cancelled.
    void answered(PolkitConversation::Verdict verdict);

private:
    void readEvents();
    void finished(int exitCode, QProcess::ExitStatus status);

    QProcess *m_process;
    QByteArray m_pending;
};
