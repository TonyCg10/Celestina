// R8-P-A. What may authorize an action, and everything that may not.
//
// The cases here are deliberately about denial. An authentication agent's
// dangerous defect is never "it rejected a good password" — that is visible
// and survivable — it is any path that reports "authenticated" for a reason
// other than the system helper saying so. So the conversation is driven
// through success, denial, cancellation and every way the question can fail to
// be asked, and the boundary is checked in the direction that matters: nothing
// but exit code 0 becomes `Authenticated`.
//
// The real helper is not exercised here and must not be: a regression that
// needed the author's actual password would either be skipped forever or hold
// one in the repository. `VAL-R8` is where a real `pkexec` meets a real
// password. What this proves is the contract around it.

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

#include "polkitconversation.h"

namespace {

// A stand-in for `celestina-polkit-converse` that speaks the same line
// protocol and exits with whatever it was told to, so the parent's own
// decisions can be examined without polkit. It also records what reached it,
// which is how the "the response went down the pipe and nowhere else" claim is
// checked rather than asserted.
QString writeFakeConverse(const QDir &dir, const QString &body)
{
    const QString path =
        dir.filePath(QStringLiteral("celestina-polkit-converse"));
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Text))
        return QString();
    file.write(body.toUtf8());
    file.close();
    file.setPermissions(QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
    return path;
}

} // namespace

class PolkitConversationTest : public QObject
{
    Q_OBJECT

private slots:
    void init();
    void cleanup();

    void anAuthenticatedExitIsTheOnlyAuthorization();
    void aDeniedAttemptIsDeniedAndSaysSo();
    void anUnavailableChildNeverAuthenticates();
    void aCrashedChildNeverAuthenticates();
    void anUnknownExitCodeNeverAuthenticates();
    void aCancelledConversationAnswersNothing();
    void thePromptsArriveExactlyAsPamWroteThem();
    void theResponseReachesOnlyTheChildsInput();
    void aSecondConversationWhileBusyIsRefused();

private:
    QTemporaryDir *m_dir = nullptr;
};

void PolkitConversationTest::init()
{
    m_dir = new QTemporaryDir();
    QVERIFY(m_dir->isValid());
}

void PolkitConversationTest::cleanup()
{
    qunsetenv("CELESTINA_POLKIT_CONVERSE");
    delete m_dir;
    m_dir = nullptr;
}

// The one path that authorizes an action, and it is reached only by the
// child's own success.
void PolkitConversationTest::anAuthenticatedExitIsTheOnlyAuthorization()
{
    const QString converse = writeFakeConverse(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\nread cookie\n"
                       "printf 'secret Password%%3A\\n'\nread answer\nexit 0\n"));
    QVERIFY(!converse.isEmpty());
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitConversation conversation;
    QSignalSpy asked(&conversation, &PolkitConversation::secretRequested);
    QSignalSpy spy(&conversation, &PolkitConversation::answered);
    conversation.start(QStringLiteral("someone"), QStringLiteral("cookie-1"));

    QVERIFY(asked.wait(4000));
    conversation.respond(QStringLiteral("whatever"));

    QVERIFY(spy.wait(4000));
    QCOMPARE(spy.count(), 1);
    QCOMPARE(spy.at(0).at(0).value<PolkitConversation::Verdict>(),
             PolkitConversation::Verdict::Authenticated);
}

void PolkitConversationTest::aDeniedAttemptIsDeniedAndSaysSo()
{
    const QString converse = writeFakeConverse(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\nread cookie\nexit 1\n"));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitConversation conversation;
    QSignalSpy spy(&conversation, &PolkitConversation::answered);
    conversation.start(QStringLiteral("someone"), QStringLiteral("cookie-1"));

    QVERIFY(spy.wait(4000));
    QCOMPARE(spy.at(0).at(0).value<PolkitConversation::Verdict>(),
             PolkitConversation::Verdict::Refused);
    // A verdict and nothing else: the wording a person reads belongs to the
    // prompt, in QML, not to this seam.
    QCOMPARE(spy.at(0).size(), 1);
}

// A child that is not there is the case a careless implementation turns into
// an authorization by treating "no error" as success.
void PolkitConversationTest::anUnavailableChildNeverAuthenticates()
{
    qputenv("CELESTINA_POLKIT_CONVERSE",
            QDir(m_dir->path()).filePath(QStringLiteral("absent")).toLocal8Bit());

    PolkitConversation conversation;
    QSignalSpy spy(&conversation, &PolkitConversation::answered);
    conversation.start(QStringLiteral("someone"), QStringLiteral("cookie-1"));

    QTRY_VERIFY_WITH_TIMEOUT(spy.count() >= 1, 4000);
    QVERIFY(spy.at(0).at(0).value<PolkitConversation::Verdict>()
            != PolkitConversation::Verdict::Authenticated);
}

void PolkitConversationTest::aCrashedChildNeverAuthenticates()
{
    const QString converse = writeFakeConverse(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\nread cookie\nkill -9 $$\n"));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitConversation conversation;
    QSignalSpy spy(&conversation, &PolkitConversation::answered);
    conversation.start(QStringLiteral("someone"), QStringLiteral("cookie-1"));

    QVERIFY(spy.wait(4000));
    QCOMPARE(spy.at(0).at(0).value<PolkitConversation::Verdict>(),
             PolkitConversation::Verdict::Unavailable);
}

// Exit codes this shell does not define are not a dialect to interpret
// generously.
void PolkitConversationTest::anUnknownExitCodeNeverAuthenticates()
{
    const QString converse = writeFakeConverse(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\nread cookie\nexit 7\n"));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitConversation conversation;
    QSignalSpy spy(&conversation, &PolkitConversation::answered);
    conversation.start(QStringLiteral("someone"), QStringLiteral("cookie-1"));

    QVERIFY(spy.wait(4000));
    QVERIFY(spy.at(0).at(0).value<PolkitConversation::Verdict>()
            != PolkitConversation::Verdict::Authenticated);
}

// A dismissed prompt is not a decision. Nothing is emitted for it, so nothing
// downstream can mistake an abandoned attempt for a denial the person saw or
// an authorization they were given.
void PolkitConversationTest::aCancelledConversationAnswersNothing()
{
    const QString converse = writeFakeConverse(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\nread cookie\n"
                       "printf 'secret Password%%3A\\n'\ncat >/dev/null\nexit 0\n"));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitConversation conversation;
    QSignalSpy asked(&conversation, &PolkitConversation::secretRequested);
    QSignalSpy spy(&conversation, &PolkitConversation::answered);
    conversation.start(QStringLiteral("someone"), QStringLiteral("cookie-1"));
    QVERIFY(asked.wait(4000));

    conversation.cancel();
    QVERIFY(!conversation.isBusy());
    QTest::qWait(300);
    QCOMPARE(spy.count(), 0);
}

// What PAM asked reaches the prompt unaltered, including the characters the
// wire encoding exists to protect. A stack that asks about a specific device
// or account must be quotable exactly: a prompt this shell paraphrased would
// be telling the person something no component here is entitled to decide.
void PolkitConversationTest::thePromptsArriveExactlyAsPamWroteThem()
{
    const QString converse = writeFakeConverse(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\nread cookie\n"
                       "printf 'info Password%%20for%%20%%E2%%80%%98disk%%E2%%80%%99\\n'\n"
                       "printf 'problem two%%0Alines\\n'\n"
                       "printf 'visible Account%%3A\\n'\n"
                       "read answer\nexit 1\n"));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitConversation conversation;
    QSignalSpy info(&conversation, &PolkitConversation::informed);
    QSignalSpy problem(&conversation, &PolkitConversation::problemReported);
    QSignalSpy visible(&conversation, &PolkitConversation::visibleRequested);
    conversation.start(QStringLiteral("someone"), QStringLiteral("cookie-1"));

    QTRY_VERIFY_WITH_TIMEOUT(visible.count() >= 1, 4000);
    QCOMPARE(info.at(0).at(0).toString(),
             QStringLiteral("Password for \u2018disk\u2019"));
    // The newline survived the wire rather than splitting one message into
    // two frames, which is the whole reason the frames are encoded.
    QCOMPARE(problem.at(0).at(0).toString(), QStringLiteral("two\nlines"));
    QCOMPARE(visible.at(0).at(0).toString(), QStringLiteral("Account:"));
    conversation.cancel();
}

// The claim ADR 0005 makes about the response, checked from the child's side:
// it arrives on stdin, and the arguments the child was started with contain
// neither it nor the cookie — those are world-readable in `/proc`.
void PolkitConversationTest::theResponseReachesOnlyTheChildsInput()
{
    const QDir dir(m_dir->path());
    const QString seen = dir.filePath(QStringLiteral("seen"));
    const QString args = dir.filePath(QStringLiteral("args"));
    const QString converse = writeFakeConverse(
        dir,
        QStringLiteral("#!/bin/sh\necho \"$@\" > %1\nread cookie\n"
                       "printf 'secret Password%%3A\\n'\n"
                       "read answer\nprintf '%s' \"$answer\" > %2\nexit 1\n")
            .arg(args, seen));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitConversation conversation;
    QSignalSpy asked(&conversation, &PolkitConversation::secretRequested);
    QSignalSpy spy(&conversation, &PolkitConversation::answered);
    conversation.start(QStringLiteral("someone"),
                       QStringLiteral("cookie-abcdef"));
    QVERIFY(asked.wait(4000));
    conversation.respond(QStringLiteral("hunter2-the-secret"));
    QVERIFY(spy.wait(4000));

    QFile input(seen);
    QVERIFY(input.open(QIODevice::ReadOnly));
    QCOMPARE(QString::fromUtf8(input.readAll()).trimmed(),
             QStringLiteral("hunter2-the-secret"));

    QFile arguments(args);
    QVERIFY(arguments.open(QIODevice::ReadOnly));
    const QString commandLine = QString::fromUtf8(arguments.readAll());
    QVERIFY(!commandLine.contains(QStringLiteral("hunter2")));
    QVERIFY(!commandLine.contains(QStringLiteral("cookie-abcdef")));
    QVERIFY(commandLine.contains(QStringLiteral("--user")));
}

// polkitd may ask twice before the first prompt is done with. A second
// conversation on the same object would leave one of them talking to a pipe
// nobody reads, so it is denied rather than queued.
void PolkitConversationTest::aSecondConversationWhileBusyIsRefused()
{
    const QString converse = writeFakeConverse(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\nread cookie\n"
                       "printf 'secret Password%%3A\\n'\ncat >/dev/null\nexit 0\n"));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitConversation conversation;
    QSignalSpy asked(&conversation, &PolkitConversation::secretRequested);
    QSignalSpy spy(&conversation, &PolkitConversation::answered);
    conversation.start(QStringLiteral("someone"), QStringLiteral("cookie-1"));
    QVERIFY(asked.wait(4000));

    conversation.start(QStringLiteral("someone"), QStringLiteral("cookie-2"));
    QCOMPARE(spy.count(), 1);
    QCOMPARE(spy.at(0).at(0).value<PolkitConversation::Verdict>(),
             PolkitConversation::Verdict::Unavailable);
    conversation.cancel();
}

QTEST_MAIN(PolkitConversationTest)

#include "polkitconversation_test.moc"
