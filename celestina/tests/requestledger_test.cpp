#include <QtTest>

#include <QSignalSpy>
#include <QVariantList>
#include <QVariantMap>

#include "providerstates.h"
#include "requestledger.h"

namespace {
// A bridge that records what it was asked to send and answers with whichever id
// the case wants — including ids no JavaScript number could hold.
class Recorder final : public RequestSink
{
public:
    struct Sent {
        QString provider;
        QString verb;
        QVariantMap options;
    };

    quint64 sendRequest(
        const QString &provider,
        const QString &verb,
        const QVariantMap &options
    ) override
    {
        sent.append(Sent {provider, verb, options});
        return nextId == 0 ? 0 : nextId++;
    }

    QList<Sent> sent;
    // Zero makes every send fail, which is what a bridge with no helper does.
    quint64 nextId = 1;
};
} // namespace

// The two request contracts this shell has, and what happens to a request
// nobody is left to hear about.
//
// The ledger exists because the surface that makes a request is often destroyed
// by the very click that makes it — a menu row is a `MenuItem`, and activating
// one closes its menu. So none of this is tested through a window: it is tested
// where it lives.
class RequestLedgerTest final : public QObject
{
    Q_OBJECT

private slots:
    void anImmediateRequestIsFinishedByAcceptance();
    void aConfirmedRequestKeepsWaitingAfterAcceptance();
    void bothContractsAreEndedByAFailure();
    void aRequestThatCouldNotBeSentFailsAtOnce();
    void aRequestIsAnsweredOnlyUnderItsOwnIdentity();
    void aReplacedRequestCannotBeAnsweredByTheOneItReplaced();
    void anIdTooLargeForADoubleSurvivesAsItself();
    void aLostGenerationEndsEveryWaitAndOnlyThose();
    void aSettledRequestIsNotReopenedByALateFrame();
    void theLedgerIsBoundedByTheOldestTarget();
    void aFailureStaysVisibleUntilItIsActedOnAgain();
    void anUnknownContractSendsNothing();
    void aConfirmationOffTheWireSettlesTheRequestItAnswers();
};

void RequestLedgerTest::anImmediateRequestIsFinishedByAcceptance()
{
    Recorder bridge;
    RequestLedger ledger(&bridge);
    QSignalSpy changed(&ledger, &RequestLedger::changed);

    const QString id = ledger.send(
        QStringLiteral("audio"), QStringLiteral("mute-toggle"), {},
        QStringLiteral("mute-toggle"), RequestLedger::ImmediatePolicy
    );
    QCOMPARE(id, QStringLiteral("1"));
    QCOMPARE(bridge.sent.size(), 1);
    QVERIFY(ledger.isPending(QStringLiteral("audio"), QStringLiteral("mute-toggle")));

    // Nothing ever sends these verbs a `confirmed`. Waiting for one is what
    // would leave the control centre saying "asking" for the rest of the
    // session, which is the regression this contract exists to prevent.
    ledger.result(1, QStringLiteral("accepted"), QString());
    QVERIFY(!ledger.isPending(QStringLiteral("audio"), QStringLiteral("mute-toggle")));
    QCOMPARE(
        ledger.stateOf(QStringLiteral("audio"), QStringLiteral("mute-toggle"))
            .value(QStringLiteral("state")).toString(),
        RequestLedger::ConfirmedState
    );
    QVERIFY(changed.count() >= 2);
}

void RequestLedgerTest::aConfirmationOffTheWireSettlesTheRequestItAnswers()
{
    // `aConfirmedRequestKeepsWaitingAfterAcceptance` drives the ledger directly, so it cannot
    // see whether a real frame ever reaches it. The parser used to reject `confirmed` as an
    // unknown state, which left a request pending until it expired even though the machine had
    // already changed. This drives the same decision the provider client makes on each line.
    const auto deliver = [](RequestLedger &ledger, const QByteArray &line) {
        const ProviderMessage message = parseProviderMessage(line);
        if (effectOf(message) != FrameEffect::Answer)
            return false;
        bool parsed = false;
        const quint64 requestId = message.requestId.toULongLong(&parsed);
        if (!parsed)
            return false;
        ledger.result(requestId, message.state, message.reason);
        return true;
    };

    Recorder bridge;
    RequestLedger ledger(&bridge);

    ledger.send(
        QStringLiteral("melibea"), QStringLiteral("minimize"), {},
        QStringLiteral("minimize:42"), RequestLedger::ConfirmedPolicy
    );

    QVERIFY(deliver(ledger, R"({"kind":"result","id":"1","state":"accepted"})"));
    QVERIFY(ledger.isPending(QStringLiteral("melibea"), QStringLiteral("minimize:42")));

    QVERIFY(deliver(ledger, R"({"kind":"result","id":"1","state":"confirmed"})"));
    QVERIFY(!ledger.isPending(QStringLiteral("melibea"), QStringLiteral("minimize:42")));
    QCOMPARE(
        ledger.stateOf(QStringLiteral("melibea"), QStringLiteral("minimize:42"))
            .value(QStringLiteral("state")).toString(),
        RequestLedger::ConfirmedState
    );

    // An unknown state must still never reach the ledger at all.
    QVERIFY(!deliver(ledger, R"({"kind":"result","id":"1","state":"maybe"})"));
}

void RequestLedgerTest::aConfirmedRequestKeepsWaitingAfterAcceptance()
{
    Recorder bridge;
    RequestLedger ledger(&bridge);

    ledger.send(
        QStringLiteral("network"), QStringLiteral("activate-saved"), {},
        QStringLiteral("activate-saved:9f1c-1"), RequestLedger::ConfirmedPolicy
    );

    // The helper ran a tool. Nothing has observed the machine yet.
    ledger.result(1, QStringLiteral("accepted"), QString());
    QVERIFY(ledger.isPending(QStringLiteral("network"), QStringLiteral("activate-saved:9f1c-1")));

    ledger.result(1, QStringLiteral("confirmed"), QString());
    QVERIFY(!ledger.isPending(QStringLiteral("network"), QStringLiteral("activate-saved:9f1c-1")));
    QCOMPARE(
        ledger.stateOf(QStringLiteral("network"), QStringLiteral("activate-saved:9f1c-1"))
            .value(QStringLiteral("state")).toString(),
        RequestLedger::ConfirmedState
    );
}

void RequestLedgerTest::bothContractsAreEndedByAFailure()
{
    Recorder bridge;
    RequestLedger ledger(&bridge);

    ledger.send(
        QStringLiteral("audio"), QStringLiteral("mute-toggle"), {},
        QStringLiteral("mute-toggle"), RequestLedger::ImmediatePolicy
    );
    ledger.send(
        QStringLiteral("bluetooth"), QStringLiteral("set-powered"), {},
        QStringLiteral("set-powered"), RequestLedger::ConfirmedPolicy
    );

    // The helper's own reason is English by contract; it is logged, and the
    // ledger carries a typed cause instead so no surface has to render it.
    QTest::ignoreMessage(
        QtWarningMsg,
        "Celestina's provider request failed: the tool refused the request"
    );
    ledger.result(1, QStringLiteral("failed"), QStringLiteral("the tool refused the request"));
    QTest::ignoreMessage(
        QtWarningMsg,
        "Celestina's provider request failed: the tool refused the request"
    );
    ledger.result(2, QStringLiteral("failed"), QStringLiteral("the tool refused the request"));

    for (const auto &pair : {
             std::pair {QStringLiteral("audio"), QStringLiteral("mute-toggle")},
             std::pair {QStringLiteral("bluetooth"), QStringLiteral("set-powered")},
         }) {
        QVERIFY(!ledger.isPending(pair.first, pair.second));
        const QVariantMap state = ledger.stateOf(pair.first, pair.second);
        QCOMPARE(state.value(QStringLiteral("state")).toString(), RequestLedger::FailedState);
        QCOMPARE(state.value(QStringLiteral("cause")).toString(), RequestLedger::ReportedCause);
    }
}

void RequestLedgerTest::aRequestThatCouldNotBeSentFailsAtOnce()
{
    Recorder bridge;
    bridge.nextId = 0;
    RequestLedger ledger(&bridge);

    const QString id = ledger.send(
        QStringLiteral("network"), QStringLiteral("refresh"), {},
        QStringLiteral("refresh"), RequestLedger::ConfirmedPolicy
    );

    // No id to wait under, so this is a failure now rather than a wait that
    // will never end.
    QVERIFY(id.isEmpty());
    QVERIFY(!ledger.isPending(QStringLiteral("network"), QStringLiteral("refresh")));
    QCOMPARE(
        ledger.stateOf(QStringLiteral("network"), QStringLiteral("refresh"))
            .value(QStringLiteral("cause")).toString(),
        RequestLedger::UnsentCause
    );
}

void RequestLedgerTest::aRequestIsAnsweredOnlyUnderItsOwnIdentity()
{
    Recorder bridge;
    RequestLedger ledger(&bridge);

    ledger.send(
        QStringLiteral("bluetooth"), QStringLiteral("connect-known"), {},
        QStringLiteral("device:AA"), RequestLedger::ConfirmedPolicy
    );
    ledger.send(
        QStringLiteral("bluetooth"), QStringLiteral("connect-known"), {},
        QStringLiteral("device:BB"), RequestLedger::ConfirmedPolicy
    );
    // The same target name under another provider is another request.
    ledger.send(
        QStringLiteral("network"), QStringLiteral("refresh"), {},
        QStringLiteral("refresh"), RequestLedger::ConfirmedPolicy
    );
    ledger.send(
        QStringLiteral("bluetooth"), QStringLiteral("refresh"), {},
        QStringLiteral("refresh"), RequestLedger::ConfirmedPolicy
    );

    ledger.result(2, QStringLiteral("confirmed"), QString());
    QVERIFY(ledger.isPending(QStringLiteral("bluetooth"), QStringLiteral("device:AA")));
    QVERIFY(!ledger.isPending(QStringLiteral("bluetooth"), QStringLiteral("device:BB")));

    ledger.result(3, QStringLiteral("confirmed"), QString());
    QVERIFY(!ledger.isPending(QStringLiteral("network"), QStringLiteral("refresh")));
    QVERIFY(ledger.isPending(QStringLiteral("bluetooth"), QStringLiteral("refresh")));

    // A result for an id nobody here asked about changes nothing at all.
    ledger.result(999, QStringLiteral("confirmed"), QString());
    QVERIFY(ledger.isPending(QStringLiteral("bluetooth"), QStringLiteral("device:AA")));
}

void RequestLedgerTest::aReplacedRequestCannotBeAnsweredByTheOneItReplaced()
{
    Recorder bridge;
    RequestLedger ledger(&bridge);

    ledger.send(
        QStringLiteral("bluetooth"), QStringLiteral("connect-known"), {},
        QStringLiteral("device:AA"), RequestLedger::ConfirmedPolicy
    );
    // The opposite action on the same device, before the first was answered.
    ledger.send(
        QStringLiteral("bluetooth"), QStringLiteral("disconnect-known"), {},
        QStringLiteral("device:AA"), RequestLedger::ConfirmedPolicy
    );

    // The first request's answer, arriving late. It belongs to nothing now.
    ledger.result(1, QStringLiteral("confirmed"), QString());
    QVERIFY(ledger.isPending(QStringLiteral("bluetooth"), QStringLiteral("device:AA")));

    ledger.result(2, QStringLiteral("confirmed"), QString());
    QVERIFY(!ledger.isPending(QStringLiteral("bluetooth"), QStringLiteral("device:AA")));
    // One target, one entry: replacing does not accumulate.
    QCOMPARE(ledger.failures(QStringLiteral("bluetooth")).size(), 0);
}

// A `quint64` past 2^53 cannot round-trip through a JavaScript number, so it
// never becomes one. The identity stays here and crosses to QML as decimal.
void RequestLedgerTest::anIdTooLargeForADoubleSurvivesAsItself()
{
    Recorder bridge;
    // Two consecutive ids that a double cannot tell apart.
    bridge.nextId = 9007199254740993ULL;
    RequestLedger ledger(&bridge);

    const QString first = ledger.send(
        QStringLiteral("network"), QStringLiteral("refresh"), {},
        QStringLiteral("refresh"), RequestLedger::ConfirmedPolicy
    );
    const QString second = ledger.send(
        QStringLiteral("bluetooth"), QStringLiteral("refresh"), {},
        QStringLiteral("refresh"), RequestLedger::ConfirmedPolicy
    );

    QCOMPARE(first, QStringLiteral("9007199254740993"));
    QCOMPARE(second, QStringLiteral("9007199254740994"));
    // Both survive a double's rounding, which would have merged them.
    QVERIFY(first != second);
    QCOMPARE(QString::number(static_cast<quint64>(static_cast<double>(9007199254740993ULL))),
             QStringLiteral("9007199254740992"));

    // And each is answered under its own identity, not its neighbour's.
    ledger.result(9007199254740994ULL, QStringLiteral("confirmed"), QString());
    QVERIFY(ledger.isPending(QStringLiteral("network"), QStringLiteral("refresh")));
    QVERIFY(!ledger.isPending(QStringLiteral("bluetooth"), QStringLiteral("refresh")));
}

void RequestLedgerTest::aLostGenerationEndsEveryWaitAndOnlyThose()
{
    Recorder bridge;
    RequestLedger ledger(&bridge);

    ledger.send(
        QStringLiteral("audio"), QStringLiteral("mute-toggle"), {},
        QStringLiteral("mute-toggle"), RequestLedger::ImmediatePolicy
    );
    ledger.send(
        QStringLiteral("network"), QStringLiteral("refresh"), {},
        QStringLiteral("refresh"), RequestLedger::ConfirmedPolicy
    );
    // Already answered before the helper went: its report is history, not a
    // wait, and must survive as what it was.
    ledger.result(1, QStringLiteral("accepted"), QString());

    ledger.generationLost();

    QVERIFY(!ledger.isPending(QStringLiteral("network"), QStringLiteral("refresh")));
    QCOMPARE(
        ledger.stateOf(QStringLiteral("network"), QStringLiteral("refresh"))
            .value(QStringLiteral("cause")).toString(),
        RequestLedger::GenerationLostCause
    );
    QCOMPARE(
        ledger.stateOf(QStringLiteral("audio"), QStringLiteral("mute-toggle"))
            .value(QStringLiteral("state")).toString(),
        RequestLedger::ConfirmedState
    );

    // A result from the generation that died answers nothing.
    ledger.result(2, QStringLiteral("confirmed"), QString());
    QCOMPARE(
        ledger.stateOf(QStringLiteral("network"), QStringLiteral("refresh"))
            .value(QStringLiteral("state")).toString(),
        RequestLedger::FailedState
    );
}

void RequestLedgerTest::aSettledRequestIsNotReopenedByALateFrame()
{
    Recorder bridge;
    RequestLedger ledger(&bridge);

    ledger.send(
        QStringLiteral("network"), QStringLiteral("refresh"), {},
        QStringLiteral("refresh"), RequestLedger::ConfirmedPolicy
    );
    ledger.result(1, QStringLiteral("confirmed"), QString());

    // The helper repeating itself, or a duplicate frame. The request is done.
    ledger.result(1, QStringLiteral("failed"), QStringLiteral("too late"));
    QCOMPARE(
        ledger.stateOf(QStringLiteral("network"), QStringLiteral("refresh"))
            .value(QStringLiteral("state")).toString(),
        RequestLedger::ConfirmedState
    );
}

void RequestLedgerTest::theLedgerIsBoundedByTheOldestTarget()
{
    Recorder bridge;
    RequestLedger ledger(&bridge);

    for (int index = 0; index < RequestLedger::maxEntries + 10; ++index) {
        ledger.send(
            QStringLiteral("network"), QStringLiteral("activate-saved"), {},
            QStringLiteral("activate-saved:%1").arg(index), RequestLedger::ConfirmedPolicy
        );
    }

    // The oldest targets are gone; the newest are all there.
    QVERIFY(ledger.stateOf(QStringLiteral("network"), QStringLiteral("activate-saved:0")).isEmpty());
    QVERIFY(!ledger.stateOf(
        QStringLiteral("network"),
        QStringLiteral("activate-saved:%1").arg(RequestLedger::maxEntries + 9)
    ).isEmpty());

    // Reopening a menu changes nothing here: the same targets are reused, so
    // repeated use does not grow it either.
    for (int index = 0; index < 200; ++index) {
        ledger.send(
            QStringLiteral("network"), QStringLiteral("refresh"), {},
            QStringLiteral("refresh"), RequestLedger::ConfirmedPolicy
        );
    }
    QVERIFY(ledger.failures(QStringLiteral("network")).size() <= RequestLedger::maxEntries);
}

// A row that goes away — a profile deleted, a device unpaired — must not take
// its failure with it silently.
void RequestLedgerTest::aFailureStaysVisibleUntilItIsActedOnAgain()
{
    Recorder bridge;
    RequestLedger ledger(&bridge);

    ledger.send(
        QStringLiteral("bluetooth"), QStringLiteral("connect-known"), {},
        QStringLiteral("device:AA"), RequestLedger::ConfirmedPolicy
    );
    QTest::ignoreMessage(QtWarningMsg, "Celestina's provider request failed: gone");
    ledger.result(1, QStringLiteral("failed"), QStringLiteral("gone"));

    const QVariantList failures = ledger.failures(QStringLiteral("bluetooth"));
    QCOMPARE(failures.size(), 1);
    QCOMPARE(
        failures.first().toMap().value(QStringLiteral("target")).toString(),
        QStringLiteral("device:AA")
    );
    // Another provider's list is its own.
    QCOMPARE(ledger.failures(QStringLiteral("network")).size(), 0);

    ledger.forget(QStringLiteral("bluetooth"), QStringLiteral("device:AA"));
    QCOMPARE(ledger.failures(QStringLiteral("bluetooth")).size(), 0);
    QVERIFY(ledger.stateOf(QStringLiteral("bluetooth"), QStringLiteral("device:AA")).isEmpty());
}

void RequestLedgerTest::anUnknownContractSendsNothing()
{
    Recorder bridge;
    RequestLedger ledger(&bridge);

    QTest::ignoreMessage(
        QtWarningMsg,
        "Celestina refused a request with no known contract: \"eventually\""
    );
    const QString id = ledger.send(
        QStringLiteral("network"), QStringLiteral("refresh"), {},
        QStringLiteral("refresh"), QStringLiteral("eventually")
    );

    QVERIFY(id.isEmpty());
    // Refused before anything was sent, so nothing reached the helper.
    QCOMPARE(bridge.sent.size(), 0);
    QVERIFY(ledger.stateOf(QStringLiteral("network"), QStringLiteral("refresh")).isEmpty());
}

QTEST_MAIN(RequestLedgerTest)
#include "requestledger_test.moc"
