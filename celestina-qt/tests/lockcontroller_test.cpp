// R6-D. What may sleep this machine, and everything that may not.
//
// The defect this guards against has one shape: the session suspends while the
// screen is still uncovered, and wakes up open. So every case here drives the
// sequence to a point where a careless implementation would suspend anyway —
// a lock that never confirms, a lock that dies mid-start, a lock binary that
// is not there — and asserts that the answer was a refusal.
//
// logind is not called: a regression that really suspended the machine could
// only be run once. What is checked is the decision *before* that call, which
// is where the rule lives.

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

#include "lockcontroller.h"

namespace {

// A stand-in for `celestina-lock`. Whatever it is told to do, it does on
// stdout, which is the only channel the controller trusts for "the screen is
// covered".
QString writeFakeLock(const QDir &dir, const QString &body)
{
    const QString path = dir.filePath(QStringLiteral("celestina-lock"));
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Text))
        return QString();
    file.write(body.toUtf8());
    file.close();
    file.setPermissions(QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
    return path;
}

} // namespace

class LockControllerTest : public QObject
{
    Q_OBJECT

private slots:
    void init();
    void cleanup();

    void aStartedLockIsNotYetALockedSession();
    void theConfirmedLineIsWhatLocks();
    void aMissingLockBinaryRefusesToSuspend();
    void aLockThatNeverConfirmsNeverSuspends();
    void aLockThatDiesBeforeCoveringRefusesToSuspend();
    void aLockThatNeverReadsItsBackdropStillCovers();
    void theBackdropCarriesOnlyAbsolutePaths();

private:
    QTemporaryDir *m_dir = nullptr;
};

void LockControllerTest::init()
{
    m_dir = new QTemporaryDir();
    QVERIFY(m_dir->isValid());
}

void LockControllerTest::cleanup()
{
    qunsetenv("CELESTINA_LOCK");
    delete m_dir;
    m_dir = nullptr;
}

// Starting is not covering. Anything that treated a running process as a
// locked session would suspend into a race with the compositor.
void LockControllerTest::aStartedLockIsNotYetALockedSession()
{
    const QString lock = writeFakeLock(
        QDir(m_dir->path()), QStringLiteral("#!/bin/sh\nsleep 5\n"));
    qputenv("CELESTINA_LOCK", lock.toLocal8Bit());

    LockController controller;
    QVERIFY(controller.lock());
    QVERIFY(controller.isStarting());
    QVERIFY(!controller.isLocked());
}

void LockControllerTest::theConfirmedLineIsWhatLocks()
{
    const QString lock = writeFakeLock(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\necho locked\nsleep 5\n"));
    qputenv("CELESTINA_LOCK", lock.toLocal8Bit());

    LockController controller;
    QSignalSpy spy(&controller, &LockController::lockedChanged);
    QVERIFY(controller.lock());
    QVERIFY(spy.wait(4000));
    QVERIFY(controller.isLocked());
}

void LockControllerTest::aMissingLockBinaryRefusesToSuspend()
{
    qputenv("CELESTINA_LOCK",
            QDir(m_dir->path()).filePath(QStringLiteral("absent")).toLocal8Bit());

    LockController controller;
    QString answer;
    bool answered = false;
    controller.lockAndSuspend([&](const QString &failure) {
        answer = failure;
        answered = true;
    });

    QVERIFY(answered);
    // A refusal carries a reason; an empty answer would mean "suspended".
    QVERIFY(!answer.isEmpty());
}

// The case a timeout would get wrong. A lock that is up but never confirms
// leaves the screen possibly uncovered, and there is no elapsed time that
// makes suspending safe.
void LockControllerTest::aLockThatNeverConfirmsNeverSuspends()
{
    const QString lock = writeFakeLock(
        QDir(m_dir->path()), QStringLiteral("#!/bin/sh\nsleep 30\n"));
    qputenv("CELESTINA_LOCK", lock.toLocal8Bit());

    LockController controller;
    bool answered = false;
    controller.lockAndSuspend([&](const QString &) { answered = true; });

    // Long enough that any "give up and sleep" timer would have fired.
    QTest::qWait(2500);
    QVERIFY(!controller.isLocked());
    QVERIFY(!answered);
}

void LockControllerTest::aLockThatDiesBeforeCoveringRefusesToSuspend()
{
    const QString lock = writeFakeLock(
        QDir(m_dir->path()), QStringLiteral("#!/bin/sh\nexit 2\n"));
    qputenv("CELESTINA_LOCK", lock.toLocal8Bit());

    LockController controller;
    QString answer;
    bool answered = false;
    controller.lockAndSuspend([&](const QString &failure) {
        answer = failure;
        answered = true;
    });

    QTRY_VERIFY_WITH_TIMEOUT(answered, 4000);
    QVERIFY(!answer.isEmpty());
    QVERIFY(!controller.isLocked());
}

// The backdrop is decoration and the cover is not. A lock that never reads the
// wallpaper line — because it is busy covering the screen, or because it does
// not care — must still confirm. Anything that waited for this hand-off would
// have made an ornament into a precondition for covering the session, and the
// failure would appear as a screen that stays uncovered.
void LockControllerTest::aLockThatNeverReadsItsBackdropStillCovers()
{
    const QString lock = writeFakeLock(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\necho locked\nsleep 5\n"));
    qputenv("CELESTINA_LOCK", lock.toLocal8Bit());

    LockController controller;
    controller.setBackdrop({{QStringLiteral("DP-1"),
                             QStringLiteral("/usr/share/backgrounds/one.png")}});

    QSignalSpy spy(&controller, &LockController::lockedChanged);
    QVERIFY(controller.lock());
    QVERIFY(spy.wait(4000));
    QVERIFY(controller.isLocked());
}

// What actually crosses the pipe, read back by the lock's stand-in. A relative
// path would name a different file in a process with a different working
// directory, so it is dropped rather than resolved here.
void LockControllerTest::theBackdropCarriesOnlyAbsolutePaths()
{
    const QDir dir(m_dir->path());
    const QString received = dir.filePath(QStringLiteral("received"));
    const QString lock = writeFakeLock(
        dir,
        QStringLiteral("#!/bin/sh\ncat > '%1'\necho locked\nsleep 5\n")
            .arg(received));
    qputenv("CELESTINA_LOCK", lock.toLocal8Bit());

    LockController controller;
    controller.setBackdrop({
        {QStringLiteral("DP-1"), QStringLiteral("/usr/share/one.png")},
        {QStringLiteral("DP-2"), QStringLiteral("relative/two.png")},
        {QStringLiteral("HDMI-A-1"), QString()},
    });
    QVERIFY(controller.lock());

    QFile file(received);
    QTRY_VERIFY_WITH_TIMEOUT(file.exists() && file.size() > 0, 4000);
    QVERIFY(file.open(QIODevice::ReadOnly));
    const QByteArray line = file.readAll();
    file.close();

    QVERIFY(line.endsWith('\n'));
    const QJsonObject payload = QJsonDocument::fromJson(line).object();
    QCOMPARE(payload.value(QStringLiteral("version")).toInt(), 1);
    const QJsonObject wallpapers =
        payload.value(QStringLiteral("wallpapers")).toObject();
    QCOMPARE(wallpapers.size(), 1);
    QCOMPARE(wallpapers.value(QStringLiteral("DP-1")).toString(),
             QStringLiteral("/usr/share/one.png"));
    QVERIFY(!wallpapers.contains(QStringLiteral("DP-2")));
    QVERIFY(!wallpapers.contains(QStringLiteral("HDMI-A-1")));
}

QTEST_MAIN(LockControllerTest)
#include "lockcontroller_test.moc"
