// LOCK-1-C. What must happen after PAM says yes, and what must not.
//
// The retreat that plays before the session is uncovered is decoration, and
// this is where that is enforced rather than merely intended. Each case drives
// the sequence to a point where an implementation that waited on the animation
// would strand somebody: a retreat nobody handles, a retreat whose handler
// hangs, a verdict repeated. In every one of them the session must still be
// uncovered, exactly once.
//
// The opposite direction is checked too, because it is the one `ADR 0004`
// actually cares about: nothing here may uncover a session that was never
// authenticated.

#include <QCoreApplication>
#include <QElapsedTimer>
#include <QSignalSpy>
#include <QTest>

#include "lock/lockuncover.h"

namespace {
// Long enough to measure ordering against, short enough to keep the suite quick.
constexpr int ceilingMs = 120;
} // namespace

class LockUncoverTest : public QObject
{
    Q_OBJECT

private slots:
    void nothingIsUncoveredUntilItBegins();
    void theRetreatIsAskedForFirst();
    void aRetreatNobodyHandlesStillUncovers();
    void aRetreatHandlerThatHangsStillUncovers();
    void beginningTwiceUncoversOnce();
};

// The class holds no verdict and must never invent one. Until something that
// does have a verdict calls `begin`, this emits nothing at all.
void LockUncoverTest::nothingIsUncoveredUntilItBegins()
{
    LockUncover uncover(ceilingMs);
    QSignalSpy uncovered(&uncover, &LockUncover::uncover);
    QSignalSpy retreated(&uncover, &LockUncover::retreat);

    QVERIFY(!uncover.hasBegun());
    // Several times the ceiling. A refusal, or no answer at all, leaves the
    // session covered for as long as it likes.
    QTest::qWait(ceilingMs * 4);

    QCOMPARE(uncovered.count(), 0);
    QCOMPARE(retreated.count(), 0);
}

// The backdrop is asked to travel before the release is due, or there would be
// nothing for the extra covered time to buy.
void LockUncoverTest::theRetreatIsAskedForFirst()
{
    LockUncover uncover(ceilingMs);
    QSignalSpy retreated(&uncover, &LockUncover::retreat);
    QSignalSpy uncovered(&uncover, &LockUncover::uncover);

    QElapsedTimer elapsed;
    elapsed.start();
    uncover.begin();

    // Immediately, and not on the timer.
    QCOMPARE(retreated.count(), 1);
    QCOMPARE(uncovered.count(), 0);

    QVERIFY(uncovered.wait(ceilingMs * 8));
    QCOMPARE(uncovered.count(), 1);
    // The session really did stay covered while the retreat played. A release
    // that fired at once would mean the compositor revealed the session mid
    // travel, which is the seam this unit exists to remove.
    QVERIFY(elapsed.elapsed() >= ceilingMs);
}

// The case a "release when the animation finishes" design gets wrong. Nothing
// is connected to `retreat` at all, so nothing will ever report it finished.
void LockUncoverTest::aRetreatNobodyHandlesStillUncovers()
{
    LockUncover uncover(ceilingMs);
    QSignalSpy uncovered(&uncover, &LockUncover::uncover);

    uncover.begin();

    QVERIFY(uncovered.wait(ceilingMs * 8));
    QCOMPARE(uncovered.count(), 1);
}

// And the worse version of it: a handler that never returns control in time —
// a stalled renderer, a cover that blocks. The person typed the right
// passphrase, so the session is theirs whatever the surface is doing.
void LockUncoverTest::aRetreatHandlerThatHangsStillUncovers()
{
    LockUncover uncover(ceilingMs);
    QSignalSpy uncovered(&uncover, &LockUncover::uncover);

    QObject::connect(&uncover, &LockUncover::retreat, &uncover, []() {
        // Longer than the ceiling, on the same thread the timer will fire on.
        QThread::msleep(ceilingMs * 2);
    });

    uncover.begin();

    QVERIFY(uncovered.wait(ceilingMs * 8));
    QCOMPARE(uncovered.count(), 1);
}

// A verdict that arrives twice — a repeated signal, a retried unlock — must not
// produce two uncoverings or restart the clock.
void LockUncoverTest::beginningTwiceUncoversOnce()
{
    LockUncover uncover(ceilingMs);
    QSignalSpy uncovered(&uncover, &LockUncover::uncover);
    QSignalSpy retreated(&uncover, &LockUncover::retreat);

    uncover.begin();
    uncover.begin();
    uncover.begin();

    QCOMPARE(retreated.count(), 1);
    QVERIFY(uncovered.wait(ceilingMs * 8));
    QTest::qWait(ceilingMs * 3);
    QCOMPARE(uncovered.count(), 1);
}

QTEST_MAIN(LockUncoverTest)
#include "lockuncover_test.moc"
