import QtQuick
import QtTest
import "../qml" as Desktop

// The card must keep measuring its own menu.
//
// The performance menu rebuilds its complete entry list on every provider
// reading, which tears down and recreates every row. If the card measures that
// list while it is incomplete and never measures again, the surface stays
// short by whatever it missed and the last rows are clipped for as long as it
// is open — the defect the author recorded on `Rendimiento`, where the tools
// row disappeared a while after opening.
TestCase {
    id: testCase

    name: "PerformanceMenuCard"
    when: windowShown

    QtObject {
        id: fakeSource

        property bool available: true
        property var providers: ({})
        property var requests: null
        property int revision: 0

        signal changed()
        signal commandResult(int requestId, string state, string reason)

        function publish(next) {
            fakeSource.providers = next;
            fakeSource.revision = fakeSource.revision + 1;
            fakeSource.changed();
        }

        function sendCommand(provider, verb, options) {
            return 1;
        }
    }

    Desktop.PerformanceMenu {
        id: menu

        outputName: "test-output"
        providerSource: fakeSource
        reducedMotion: true
    }

    function reading(cpu, ram) {
        fakeSource.publish({
            "sysmon": {"cpu": cpu, "ram": ram}
        });
    }

    function measuredRowHeights() {
        let total = menu.menu.topPadding + menu.menu.bottomPadding;
        for (let index = 0; index < menu.menu.count; ++index) {
            const item = menu.menu.itemAt(index);
            verify(item, "row " + index + " of " + menu.menu.count
                         + " is missing from the menu");
            total += item.implicitHeight;
        }
        return Math.ceil(total);
    }

    function test_the_card_keeps_every_row_across_live_readings() {
        testCase.reading(1, 6);
        tryVerify(function() { return menu.menu.count > 0; });

        // Header, the one section, and the two readings.
        const settledCount = menu.menu.count;
        compare(settledCount, 4);
        const settledHeight = menu.naturalMenuHeight;
        compare(settledHeight, testCase.measuredRowHeights());
        compare(menu.cardHeight, settledHeight);

        // A reading tick may only move the two value labels. It must not
        // recreate the rows: the tick-driven rebuild is what re-measured the
        // card against a mid-rebuild menu on the live compositor and left it
        // permanently clipped. Row identity is the proof — the same items,
        // not equal-looking replacements.
        const settledRows = [];
        for (let index = 0; index < settledCount; ++index)
            settledRows.push(menu.menu.itemAt(index));

        for (let tick = 2; tick <= 8; ++tick) {
            testCase.reading(tick, 6);
            compare(menu.menu.count, settledCount);
            for (let index = 0; index < settledCount; ++index) {
                verify(menu.menu.itemAt(index) === settledRows[index],
                       "row " + index + " was recreated by a reading tick");
            }
            compare(menu.naturalMenuHeight, settledHeight);
            compare(menu.naturalMenuHeight, testCase.measuredRowHeights());
            compare(menu.cardHeight, settledHeight);
        }

        // And the labels did move: the last tick is on screen.
        let metrics = 0;
        for (let index = 0; index < settledCount; ++index) {
            const row = menu.menu.itemAt(index);
            if (row.note.length > 0) {
                ++metrics;
                if (row.text === qsTr("Procesador"))
                    compare(row.note, qsTr("8 %"));
            }
        }
        compare(metrics, 2);
    }

    function test_a_row_appearing_later_is_measured() {
        // Losing the reading swaps the two metric rows for one unavailable
        // row; regaining it swaps them back. Both directions must leave the
        // card measuring exactly what the menu now holds.
        testCase.reading(3, 7);
        tryVerify(function() { return menu.menu.count === 4; });
        const withReading = menu.naturalMenuHeight;

        fakeSource.publish({});
        tryCompare(menu.menu, "count", 3);
        tryCompare(menu, "naturalMenuHeight", testCase.measuredRowHeights());
        verify(menu.naturalMenuHeight < withReading);

        testCase.reading(4, 8);
        tryCompare(menu.menu, "count", 4);
        tryCompare(menu, "naturalMenuHeight", withReading);
        compare(menu.cardHeight, withReading);
    }
}
