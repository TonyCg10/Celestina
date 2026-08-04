#include <QTest>

#include "sessionrequests.h"

namespace {
QVariantMap audioProviders(int volume, bool muted)
{
    QVariantMap audio;
    audio.insert(QStringLiteral("volume"), volume);
    audio.insert(QStringLiteral("muted"), muted);

    QVariantMap providers;
    providers.insert(QStringLiteral("audio"), audio);
    return providers;
}

SessionRequests::Expectation absoluteVolume(int level)
{
    return SessionRequests::Expectation {
        QStringLiteral("audio"),
        QStringLiteral("volume"),
        level,
        3000,
    };
}

SessionRequests::Expectation anyAudioChange()
{
    return SessionRequests::Expectation {
        QStringLiteral("audio"),
        QString(),
        QVariant(),
        3000,
    };
}

QStringList states(const QList<SessionRequests::Outcome> &outcomes)
{
    QStringList reported;
    reported.reserve(outcomes.size());
    for (const SessionRequests::Outcome &outcome : outcomes)
        reported.append(outcome.state);
    return reported;
}
} // namespace

class SessionRequestsTest final : public QObject
{
    Q_OBJECT

private slots:
    void aRequestIsPendingUntilSomethingIsObserved();
    void acceptanceIsNotArrival();
    void aRefusalEndsTheRequestWithItsReason();
    void anAbsoluteTargetIsConfirmedOnlyByThatValue();
    void anUnpredictableResultIsConfirmedByAnyChangeToItsProvider();
    void aLevelIsComparedAsANumberNotAsAType();
    void aRequestNothingReportsFailsWhenItsTimeIsUp();
    void aLostHelperFailsEverythingInFlight();
    void aFullTableRefusesInsteadOfForgetting();
};

void SessionRequestsTest::aRequestIsPendingUntilSomethingIsObserved()
{
    SessionRequests requests;
    QVERIFY(requests.begin(
        1, 10, QStringLiteral("volume-set"), absoluteVolume(40),
        audioProviders(30, false), 0
    ));

    QCOMPARE(states(requests.takeOutcomes()), QStringList {QStringLiteral("pending")});
    QVERIFY(!requests.isEmpty());
    // Draining twice does not report the same transition again.
    QVERIFY(requests.takeOutcomes().isEmpty());
}

void SessionRequestsTest::acceptanceIsNotArrival()
{
    SessionRequests requests;
    requests.begin(
        1, 10, QStringLiteral("volume-set"), absoluteVolume(40),
        audioProviders(30, false), 0
    );
    requests.takeOutcomes();

    requests.acknowledge(10, QStringLiteral("accepted"), QString(), 5);

    QVERIFY(requests.takeOutcomes().isEmpty());
    QVERIFY(!requests.isEmpty());
}

void SessionRequestsTest::aRefusalEndsTheRequestWithItsReason()
{
    SessionRequests requests;
    requests.begin(
        1, 10, QStringLiteral("volume-set"), absoluteVolume(40),
        audioProviders(30, false), 0
    );
    requests.takeOutcomes();

    requests.acknowledge(
        10,
        QStringLiteral("failed"),
        QStringLiteral("wpctl refused to volume-set"),
        5
    );

    const QList<SessionRequests::Outcome> outcomes = requests.takeOutcomes();
    QCOMPARE(outcomes.size(), 1);
    QCOMPARE(outcomes.first().state, QStringLiteral("failed"));
    QCOMPARE(outcomes.first().verb, QStringLiteral("volume-set"));
    QVERIFY(outcomes.first().reason.contains(QStringLiteral("wpctl")));
    QVERIFY(requests.isEmpty());
}

void SessionRequestsTest::anAbsoluteTargetIsConfirmedOnlyByThatValue()
{
    SessionRequests requests;
    requests.begin(
        1, 10, QStringLiteral("volume-set"), absoluteVolume(40),
        audioProviders(30, false), 0
    );
    requests.takeOutcomes();

    // The device moved, but not to what was asked for: still pending.
    requests.applyProviders(audioProviders(35, false), 10);
    QVERIFY(requests.takeOutcomes().isEmpty());

    requests.applyProviders(audioProviders(40, false), 20);
    const QList<SessionRequests::Outcome> outcomes = requests.takeOutcomes();
    QCOMPARE(outcomes.size(), 1);
    QCOMPARE(outcomes.first().state, QStringLiteral("confirmed"));
    QVERIFY(outcomes.first().reason.isEmpty());
    QVERIFY(requests.isEmpty());
}

void SessionRequestsTest::anUnpredictableResultIsConfirmedByAnyChangeToItsProvider()
{
    SessionRequests requests;
    requests.begin(
        1, 10, QStringLiteral("mute-toggle"), anyAudioChange(),
        audioProviders(30, false), 0
    );
    requests.takeOutcomes();

    // Another provider publishing says nothing about this one.
    QVariantMap unrelated = audioProviders(30, false);
    unrelated.insert(QStringLiteral("power"), QVariantMap {
        {QStringLiteral("active"), QStringLiteral("balanced")}
    });
    requests.applyProviders(unrelated, 10);
    QVERIFY(requests.takeOutcomes().isEmpty());

    requests.applyProviders(audioProviders(30, true), 20);
    QCOMPARE(
        states(requests.takeOutcomes()),
        QStringList {QStringLiteral("confirmed")}
    );
}

void SessionRequestsTest::aLevelIsComparedAsANumberNotAsAType()
{
    SessionRequests requests;
    // The bus carries a level as a 64-bit integer; the helper publishes it as
    // a JSON number. They are the same reading.
    requests.begin(
        1, 10, QStringLiteral("volume-set"),
        SessionRequests::Expectation {
            QStringLiteral("audio"),
            QStringLiteral("volume"),
            QVariant(qlonglong(40)),
            3000,
        },
        audioProviders(30, false), 0
    );
    requests.takeOutcomes();

    requests.applyProviders(audioProviders(40, false), 10);
    QCOMPARE(
        states(requests.takeOutcomes()),
        QStringList {QStringLiteral("confirmed")}
    );
}

void SessionRequestsTest::aRequestNothingReportsFailsWhenItsTimeIsUp()
{
    SessionRequests requests;
    requests.begin(
        1, 10, QStringLiteral("volume-set"), absoluteVolume(40),
        audioProviders(30, false), 0
    );
    requests.acknowledge(10, QStringLiteral("accepted"), QString(), 1);
    requests.takeOutcomes();

    requests.expire(2999);
    QVERIFY(requests.takeOutcomes().isEmpty());

    requests.expire(3000);
    const QList<SessionRequests::Outcome> outcomes = requests.takeOutcomes();
    QCOMPARE(outcomes.size(), 1);
    QCOMPARE(outcomes.first().state, QStringLiteral("failed"));
    QVERIFY(!outcomes.first().reason.isEmpty());
    QVERIFY(requests.isEmpty());
}

void SessionRequestsTest::aLostHelperFailsEverythingInFlight()
{
    SessionRequests requests;
    requests.begin(
        1, 10, QStringLiteral("volume-set"), absoluteVolume(40),
        audioProviders(30, false), 0
    );
    requests.begin(
        2, 11, QStringLiteral("mute-toggle"), anyAudioChange(),
        audioProviders(30, false), 0
    );
    requests.takeOutcomes();

    requests.failAll(QStringLiteral("the provider helper is unavailable"));

    const QList<SessionRequests::Outcome> outcomes = requests.takeOutcomes();
    QCOMPARE(outcomes.size(), 2);
    QCOMPARE(
        states(outcomes),
        (QStringList {QStringLiteral("failed"), QStringLiteral("failed")})
    );
    QVERIFY(requests.isEmpty());
}

void SessionRequestsTest::aFullTableRefusesInsteadOfForgetting()
{
    SessionRequests requests;
    for (quint64 request = 1; request <= 32; ++request) {
        QVERIFY(requests.begin(
            request, request, QStringLiteral("mute-toggle"), anyAudioChange(),
            audioProviders(30, false), 0
        ));
    }

    QVERIFY(requests.isFull());
    QVERIFY(!requests.begin(
        99, 99, QStringLiteral("mute-toggle"), anyAudioChange(),
        audioProviders(30, false), 0
    ));
}

QTEST_MAIN(SessionRequestsTest)

#include "sessionrequests_test.moc"
