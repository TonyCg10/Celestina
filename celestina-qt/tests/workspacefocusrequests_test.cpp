#include <QtTest>

#include "workspacefocusrequests.h"

namespace {
constexpr quint64 generation = 1;

WorkspaceFocusRequests::Timings testTimings()
{
    return WorkspaceFocusRequests::Timings {1000, 300, 500};
}

QHash<QString, int> activeWorkspace(const QString &output, int index)
{
    return QHash<QString, int> {{output, index}};
}
} // namespace

class WorkspaceFocusRequestsTest final : public QObject
{
    Q_OBJECT

private slots:
    void staysPendingUntilAMatchingSnapshotArrives();
    void aSnapshotForAnotherWorkspaceDoesNotConfirm();
    void aRejectedRequestFailsImmediately();
    void aSilentRequestTimesOutAsFailed();
    void terminalStatesClearAfterTheirHold();
    void aResultFromAPreviousAdapterIsIgnored();
    void adapterLossFailsEveryPendingRequest();
    void refusesADuplicateTargetAndBoundsTheTable();
};

void WorkspaceFocusRequestsTest::staysPendingUntilAMatchingSnapshotArrives()
{
    WorkspaceFocusRequests requests(testTimings());
    QVERIFY(requests.begin(1, generation, QStringLiteral("DP-1"), 3, 0));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("pending"));

    // Niri accepted the request. That is not proof it took effect.
    QVERIFY(!requests.acknowledge(1, generation, true, 10));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("pending"));

    QVERIFY(requests.applyActive(activeWorkspace(QStringLiteral("DP-1"), 3), 20));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("confirmed"));
}

void WorkspaceFocusRequestsTest::aSnapshotForAnotherWorkspaceDoesNotConfirm()
{
    WorkspaceFocusRequests requests(testTimings());
    QVERIFY(requests.begin(1, generation, QStringLiteral("DP-1"), 3, 0));

    // Same index on another output, and another index on the requested one.
    QVERIFY(!requests.applyActive(activeWorkspace(QStringLiteral("DP-2"), 3), 10));
    QVERIFY(!requests.applyActive(activeWorkspace(QStringLiteral("DP-1"), 2), 20));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("pending"));
}

void WorkspaceFocusRequestsTest::aRejectedRequestFailsImmediately()
{
    WorkspaceFocusRequests requests(testTimings());
    QVERIFY(requests.begin(1, generation, QStringLiteral("DP-1"), 3, 0));

    QVERIFY(requests.acknowledge(1, generation, false, 10));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("failed"));

    // A late snapshot cannot resurrect a request the compositor refused.
    QVERIFY(!requests.applyActive(activeWorkspace(QStringLiteral("DP-1"), 3), 20));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("failed"));
}

void WorkspaceFocusRequestsTest::aSilentRequestTimesOutAsFailed()
{
    WorkspaceFocusRequests requests(testTimings());
    QVERIFY(requests.begin(1, generation, QStringLiteral("DP-1"), 3, 0));

    QVERIFY(!requests.expire(999));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("pending"));

    QVERIFY(requests.expire(1000));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("failed"));
}

void WorkspaceFocusRequestsTest::terminalStatesClearAfterTheirHold()
{
    WorkspaceFocusRequests requests(testTimings());
    QVERIFY(requests.begin(1, generation, QStringLiteral("DP-1"), 3, 0));
    QVERIFY(requests.applyActive(activeWorkspace(QStringLiteral("DP-1"), 3), 0));

    QVERIFY(!requests.expire(299));
    QVERIFY(requests.expire(300));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QString());
    QVERIFY(requests.isEmpty());
}

void WorkspaceFocusRequestsTest::aResultFromAPreviousAdapterIsIgnored()
{
    WorkspaceFocusRequests requests(testTimings());
    QVERIFY(requests.begin(1, generation, QStringLiteral("DP-1"), 3, 0));

    QVERIFY(!requests.acknowledge(1, generation + 1, false, 10));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("pending"));

    // An id this host never issued is not an answer either.
    QVERIFY(!requests.acknowledge(42, generation, false, 10));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("pending"));
}

void WorkspaceFocusRequestsTest::adapterLossFailsEveryPendingRequest()
{
    WorkspaceFocusRequests requests(testTimings());
    QVERIFY(requests.begin(1, generation, QStringLiteral("DP-1"), 3, 0));
    QVERIFY(requests.begin(2, generation, QStringLiteral("DP-2"), 1, 0));

    QVERIFY(requests.failAll(10));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("failed"));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-2"), 1), QStringLiteral("failed"));
    QVERIFY(!requests.failAll(20));
}

void WorkspaceFocusRequestsTest::refusesADuplicateTargetAndBoundsTheTable()
{
    WorkspaceFocusRequests requests(testTimings());
    QVERIFY(requests.begin(1, generation, QStringLiteral("DP-1"), 3, 0));
    QVERIFY(!requests.begin(2, generation, QStringLiteral("DP-1"), 3, 0));

    for (int index = 1; index <= 40; ++index) {
        const bool accepted =
            requests.begin(quint64(index) + 2, generation, QStringLiteral("DP-2"), index, 0);
        // One slot is already held by the DP-1 request above.
        QCOMPARE(accepted, index <= 31);
    }

    // A held failure is replaced by a fresh request for the same workspace.
    QVERIFY(requests.acknowledge(1, generation, false, 10));
    QVERIFY(requests.begin(100, generation, QStringLiteral("DP-1"), 3, 10));
    QCOMPARE(requests.stateFor(QStringLiteral("DP-1"), 3), QStringLiteral("pending"));
}

QTEST_GUILESS_MAIN(WorkspaceFocusRequestsTest)

#include "workspacefocusrequests_test.moc"
