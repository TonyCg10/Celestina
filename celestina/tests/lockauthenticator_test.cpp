// R6-A. What may unlock this session, and everything that may not.
//
// The cases here are deliberately about refusal. A lock's dangerous defect is
// never "it rejected a good passphrase" — that is visible and survivable — it
// is any path that answers "authenticated" for a reason other than PAM saying
// so. So the verifier is driven through success, refusal, and every way the
// question can fail to be asked, and the boundary is checked in the direction
// that matters: nothing but exit code 0 becomes `Authenticated`.
//
// The real PAM stack is not exercised here and must not be: a regression that
// needed the author's actual password would either be skipped forever or
// hold one in the repository. `VAL-R6` is where a real passphrase meets a real
// stack. What this proves is the contract around it.

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QProcess>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

#include "lockauthenticator.h"

namespace {

// A stand-in for `celestina-lock-verify` that exits with whatever it was told
// to, so the parent's own decisions can be examined without PAM. It also
// records what reached it, which is how the "the passphrase went down the
// pipe and nowhere else" claim is checked rather than asserted.
QString writeFakeVerifier(const QDir &dir, const QString &body)
{
    const QString path = dir.filePath(QStringLiteral("celestina-lock-verify"));
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Text))
        return QString();
    file.write(body.toUtf8());
    file.close();
    file.setPermissions(QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
    return path;
}

} // namespace

class LockAuthenticatorTest : public QObject
{
    Q_OBJECT

private slots:
    void init();
    void cleanup();

    void anAuthenticatedExitIsTheOnlyUnlock();
    void aRefusalIsRefusedAndSaysSo();
    void anUnavailableVerifierNeverAuthenticates();
    void aCrashedVerifierNeverAuthenticates();
    void anUnknownExitCodeNeverAuthenticates();
    void theSecretReachesOnlyTheChildsInput();
    void aSecondAttemptWhileBusyIsRefused();

private:
    QTemporaryDir *m_dir = nullptr;
};

void LockAuthenticatorTest::init()
{
    m_dir = new QTemporaryDir();
    QVERIFY(m_dir->isValid());
}

void LockAuthenticatorTest::cleanup()
{
    qunsetenv("CELESTINA_LOCK_VERIFY");
    delete m_dir;
    m_dir = nullptr;
}

// The one path that opens a locked machine, and it is reached only by the
// child's own success.
void LockAuthenticatorTest::anAuthenticatedExitIsTheOnlyUnlock()
{
    const QString verifier = writeFakeVerifier(
        QDir(m_dir->path()), QStringLiteral("#!/bin/sh\ncat >/dev/null\nexit 0\n"));
    QVERIFY(!verifier.isEmpty());
    qputenv("CELESTINA_LOCK_VERIFY", verifier.toLocal8Bit());

    LockAuthenticator authenticator;
    authenticator.setUser(QStringLiteral("someone"));
    QSignalSpy spy(&authenticator, &LockAuthenticator::answered);
    authenticator.authenticate(QStringLiteral("whatever"));

    QVERIFY(spy.wait(4000));
    QCOMPARE(spy.count(), 1);
    QCOMPARE(spy.at(0).at(0).value<LockAuthenticator::Verdict>(),
             LockAuthenticator::Verdict::Authenticated);
}

void LockAuthenticatorTest::aRefusalIsRefusedAndSaysSo()
{
    const QString verifier = writeFakeVerifier(
        QDir(m_dir->path()), QStringLiteral("#!/bin/sh\ncat >/dev/null\nexit 1\n"));
    qputenv("CELESTINA_LOCK_VERIFY", verifier.toLocal8Bit());

    LockAuthenticator authenticator;
    authenticator.setUser(QStringLiteral("someone"));
    QSignalSpy spy(&authenticator, &LockAuthenticator::answered);
    authenticator.authenticate(QStringLiteral("wrong"));

    QVERIFY(spy.wait(4000));
    QCOMPARE(spy.at(0).at(0).value<LockAuthenticator::Verdict>(),
             LockAuthenticator::Verdict::Refused);
    // A verdict and nothing else: the wording a person reads belongs to the
    // lock surface, in QML, not to this seam.
    QCOMPARE(spy.at(0).size(), 1);
}

// A verifier that is not there is the case a careless implementation turns
// into an unlock by treating "no error" as success.
void LockAuthenticatorTest::anUnavailableVerifierNeverAuthenticates()
{
    qputenv("CELESTINA_LOCK_VERIFY",
            QDir(m_dir->path()).filePath(QStringLiteral("absent")).toLocal8Bit());

    LockAuthenticator authenticator;
    authenticator.setUser(QStringLiteral("someone"));
    QSignalSpy spy(&authenticator, &LockAuthenticator::answered);
    authenticator.authenticate(QStringLiteral("whatever"));

    QTRY_VERIFY_WITH_TIMEOUT(spy.count() >= 1, 4000);
    QVERIFY(spy.at(0).at(0).value<LockAuthenticator::Verdict>()
            != LockAuthenticator::Verdict::Authenticated);
}

void LockAuthenticatorTest::aCrashedVerifierNeverAuthenticates()
{
    const QString verifier = writeFakeVerifier(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\ncat >/dev/null\nkill -9 $$\n"));
    qputenv("CELESTINA_LOCK_VERIFY", verifier.toLocal8Bit());

    LockAuthenticator authenticator;
    authenticator.setUser(QStringLiteral("someone"));
    QSignalSpy spy(&authenticator, &LockAuthenticator::answered);
    authenticator.authenticate(QStringLiteral("whatever"));

    QVERIFY(spy.wait(4000));
    QCOMPARE(spy.at(0).at(0).value<LockAuthenticator::Verdict>(),
             LockAuthenticator::Verdict::Unavailable);
}

// Exit codes this shell does not define are not a dialect to interpret
// generously.
void LockAuthenticatorTest::anUnknownExitCodeNeverAuthenticates()
{
    const QString verifier = writeFakeVerifier(
        QDir(m_dir->path()), QStringLiteral("#!/bin/sh\ncat >/dev/null\nexit 7\n"));
    qputenv("CELESTINA_LOCK_VERIFY", verifier.toLocal8Bit());

    LockAuthenticator authenticator;
    authenticator.setUser(QStringLiteral("someone"));
    QSignalSpy spy(&authenticator, &LockAuthenticator::answered);
    authenticator.authenticate(QStringLiteral("whatever"));

    QVERIFY(spy.wait(4000));
    QVERIFY(spy.at(0).at(0).value<LockAuthenticator::Verdict>()
            != LockAuthenticator::Verdict::Authenticated);
}

// The claim ADR 0004 makes about the passphrase, checked from the child's
// side: it arrives on stdin, and the arguments the child was started with do
// not contain it — those are world-readable in `/proc`.
void LockAuthenticatorTest::theSecretReachesOnlyTheChildsInput()
{
    const QString seen = QDir(m_dir->path()).filePath(QStringLiteral("seen"));
    const QString args = QDir(m_dir->path()).filePath(QStringLiteral("args"));
    const QString verifier = writeFakeVerifier(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\necho \"$@\" > %1\ncat > %2\nexit 1\n")
            .arg(args, seen));
    qputenv("CELESTINA_LOCK_VERIFY", verifier.toLocal8Bit());

    LockAuthenticator authenticator;
    authenticator.setUser(QStringLiteral("someone"));
    QSignalSpy spy(&authenticator, &LockAuthenticator::answered);
    authenticator.authenticate(QStringLiteral("hunter2-the-secret"));
    QVERIFY(spy.wait(4000));

    QFile input(seen);
    QVERIFY(input.open(QIODevice::ReadOnly));
    QCOMPARE(QString::fromUtf8(input.readAll()).trimmed(),
             QStringLiteral("hunter2-the-secret"));

    QFile arguments(args);
    QVERIFY(arguments.open(QIODevice::ReadOnly));
    const QString commandLine = QString::fromUtf8(arguments.readAll());
    QVERIFY(!commandLine.contains(QStringLiteral("hunter2")));
    QVERIFY(commandLine.contains(QStringLiteral("--user")));
}

void LockAuthenticatorTest::aSecondAttemptWhileBusyIsRefused()
{
    const QString verifier = writeFakeVerifier(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\ncat >/dev/null\nsleep 1\nexit 0\n"));
    qputenv("CELESTINA_LOCK_VERIFY", verifier.toLocal8Bit());

    LockAuthenticator authenticator;
    authenticator.setUser(QStringLiteral("someone"));
    QSignalSpy spy(&authenticator, &LockAuthenticator::answered);
    authenticator.authenticate(QStringLiteral("first"));
    QVERIFY(authenticator.isBusy());

    // The second is answered immediately, and never as an unlock.
    authenticator.authenticate(QStringLiteral("second"));
    QCOMPARE(spy.count(), 1);
    QCOMPARE(spy.at(0).at(0).value<LockAuthenticator::Verdict>(),
             LockAuthenticator::Verdict::Unavailable);

    authenticator.cancel();
}

QTEST_MAIN(LockAuthenticatorTest)
#include "lockauthenticator_test.moc"
