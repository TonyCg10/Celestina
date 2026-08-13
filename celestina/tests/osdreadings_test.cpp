#include <QTest>

#include "osdreadings.h"

namespace {
QVariantMap audio(int volume, bool muted)
{
    QVariantMap values;
    values.insert(QStringLiteral("volume"), volume);
    values.insert(QStringLiteral("muted"), muted);

    QVariantMap providers;
    providers.insert(QStringLiteral("audio"), values);
    return providers;
}

QVariantMap withMic(QVariantMap providers, int volume, bool muted)
{
    QVariantMap values = providers.value(QStringLiteral("audio")).toMap();
    values.insert(QStringLiteral("micVolume"), volume);
    values.insert(QStringLiteral("micMuted"), muted);
    providers.insert(QStringLiteral("audio"), values);
    return providers;
}

QVariantMap brightness(const QVariantMap &levels)
{
    QVariantMap providers;
    providers.insert(QStringLiteral("brightness"), levels);
    return providers;
}
} // namespace

class OsdReadingsTest final : public QObject
{
    Q_OBJECT

private slots:
    void theFirstValueIsABaselineAndShowsNothing();
    void aChangedLevelIsWorthShowing();
    void aMuteIsWorthShowingEvenAtTheSameLevel();
    void theMicrophoneIsItsOwnReading();
    void aMonitorIsNamedByItsConnector();
    void aMonitorThatHasNotAnsweredIsNeverShown();
    void volumeIsReportedBeforeAMonitorWhenBothChange();
    void aLostHelperMakesTheNextValueABaselineAgain();
};

void OsdReadingsTest::theFirstValueIsABaselineAndShowsNothing()
{
    OsdReadings readings;

    QVERIFY(readings.apply(audio(40, false)).isEmpty());
    // The same value again is not news either.
    QVERIFY(readings.apply(audio(40, false)).isEmpty());
}

void OsdReadingsTest::aChangedLevelIsWorthShowing()
{
    OsdReadings readings;
    readings.apply(audio(40, false));

    const auto reading = readings.apply(audio(45, false));
    QVERIFY(!reading.isEmpty());
    QCOMPARE(reading.first().kind, QStringLiteral("volume"));
    QCOMPARE(reading.first().percent, 45);
    QVERIFY(!reading.first().muted);
    QVERIFY(reading.first().label.isEmpty());
}

void OsdReadingsTest::aMuteIsWorthShowingEvenAtTheSameLevel()
{
    OsdReadings readings;
    readings.apply(audio(40, false));

    const auto reading = readings.apply(audio(40, true));
    QVERIFY(!reading.isEmpty());
    QCOMPARE(reading.first().kind, QStringLiteral("volume"));
    // A muted device keeps the level it remembers.
    QCOMPARE(reading.first().percent, 40);
    QVERIFY(reading.first().muted);
}

void OsdReadingsTest::theMicrophoneIsItsOwnReading()
{
    OsdReadings readings;
    readings.apply(withMic(audio(40, false), 70, false));

    const auto reading = readings.apply(withMic(audio(40, false), 70, true));
    QVERIFY(!reading.isEmpty());
    QCOMPARE(reading.first().kind, QStringLiteral("microphone"));
    QCOMPARE(reading.first().percent, 70);
    QVERIFY(reading.first().muted);
}

void OsdReadingsTest::aMonitorIsNamedByItsConnector()
{
    OsdReadings readings;
    readings.apply(brightness({{QStringLiteral("DP-1"), 60},
                               {QStringLiteral("DP-2"), 60}}));

    const auto reading = readings.apply(
        brightness({{QStringLiteral("DP-1"), 60}, {QStringLiteral("DP-2"), 55}})
    );
    QVERIFY(!reading.isEmpty());
    QCOMPARE(reading.first().kind, QStringLiteral("brightness"));
    QCOMPARE(reading.first().percent, 55);
    QCOMPARE(reading.first().label, QStringLiteral("DP-2"));
}

void OsdReadingsTest::aMonitorThatHasNotAnsweredIsNeverShown()
{
    OsdReadings readings;
    // Unknown is not zero, and a monitor going from unknown to a first reading
    // is that monitor answering, not somebody changing it.
    QVERIFY(readings.apply(brightness({{QStringLiteral("DP-1"), QVariant()}}))
                .isEmpty());
    QVERIFY(readings.apply(brightness({{QStringLiteral("DP-1"), 60}}))
                .isEmpty());

    QVERIFY(!readings.apply(brightness({{QStringLiteral("DP-1"), 65}})).isEmpty());
}

void OsdReadingsTest::volumeIsReportedBeforeAMonitorWhenBothChange()
{
    OsdReadings readings;
    QVariantMap both = audio(40, false);
    both.insert(QStringLiteral("brightness"),
                QVariantMap {{QStringLiteral("DP-1"), 60}});
    readings.apply(both);

    QVariantMap moved = audio(45, false);
    moved.insert(QStringLiteral("brightness"),
                 QVariantMap {{QStringLiteral("DP-1"), 65}});

    // Both changed, so both are announced — volume first, and the monitor's
    // card right behind it instead of being silently swallowed, which is what
    // made one command that moved both raise one display.
    const auto reading = readings.apply(moved);
    QCOMPARE(reading.size(), 2);
    QCOMPARE(reading.first().kind, QStringLiteral("volume"));
    QCOMPARE(reading.last().kind, QStringLiteral("brightness"));
    QCOMPARE(reading.last().percent, 65);

    // Both new levels were recorded, so neither is announced later as though
    // it had just moved.
    QVERIFY(readings.apply(moved).isEmpty());
}

void OsdReadingsTest::aLostHelperMakesTheNextValueABaselineAgain()
{
    OsdReadings readings;
    readings.apply(audio(40, false));
    readings.forget();

    QVERIFY(readings.apply(audio(45, false)).isEmpty());
    QVERIFY(!readings.apply(audio(50, false)).isEmpty());
}

QTEST_MAIN(OsdReadingsTest)

#include "osdreadings_test.moc"
