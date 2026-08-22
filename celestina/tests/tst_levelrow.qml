import QtQuick
import QtTest
import "../qml" as Desktop

// How a level row behaves against a provider that answers at its own pace.
//
// Both of this row's consumers are slow in their own way: audio costs a
// process per move, and a monitor over DDC takes about a second. Constructed
// offscreen: this proves the pacing and what the row shows, never appearance.
TestCase {
    id: testCase

    name: "LevelRow"

    Desktop.BackdropInk {
        id: rowInk
    }

    Component {
        id: rowComponent

        Desktop.LevelRow {
            ink: rowInk
            label: "Volumen"
            level: 40
            width: 300

            property var asks: []

            onMoved: (target) => asks.push(target)
        }
    }

    function test_the_row_shows_what_it_asked_for_until_the_provider_answers() {
        const row = rowComponent.createObject(null);
        verify(row);
        compare(row.shownLevel, 40);

        row.ask(70);
        compare(row.asks, [70]);
        // The provider has said nothing yet, and the row does not spring back
        // to a reading it already knows is behind.
        compare(row.level, 40);
        compare(row.shownLevel, 70);

        // The reading arrives and is the truth again.
        row.level = 70;
        compare(row.shownLevel, 70);
        compare(row.asked, -1);
        row.destroy();
    }

    // A reading that is not the answer does not move the thumb.
    //
    // This is the difference between a slider that follows the hand and one
    // that fights it. Providers publish readings nobody asked for — a poll, or
    // the read-back of an earlier request — and they arrive mid-drag saying
    // where the device *was*. Believing them immediately is what made the
    // thumb jump back to a position the person had already left.
    function test_a_reading_that_is_not_the_answer_does_not_move_the_thumb() {
        const row = rowComponent.createObject(null);
        verify(row);

        row.ask(100);
        compare(row.shownLevel, 100);

        // The provider's own poll, describing where the device was before.
        row.level = 93;
        compare(row.shownLevel, 100, "the drag's own target still holds");

        // But the provider is still authoritative, and says so last: with
        // nothing more asked and no exact answer, its reading takes over.
        tryCompare(row, "asked", -1, 5000);
        compare(row.shownLevel, 93);
        row.destroy();
    }

    // Any reading completes the round trip, even one that answers nothing, so
    // a drag is never left waiting on a device that landed somewhere else.
    function test_a_reading_that_answers_nothing_still_releases_the_next_ask() {
        const row = rowComponent.createObject(null);
        verify(row);

        row.ask(60);
        row.ask(80);
        compare(row.asks, [60]);

        row.level = 41;

        compare(row.asks, [60, 80], "the newest target went without waiting");
        compare(row.shownLevel, 80);
        row.destroy();
    }

    // A drag crosses many positions. Only one may be in flight, and the one
    // the person let go on must be the one that lands.
    function test_a_drag_keeps_one_request_in_flight_and_sends_the_last() {
        const row = rowComponent.createObject(null);
        verify(row);

        row.ask(50);
        row.ask(55);
        row.ask(60);
        row.ask(65);
        compare(row.asks, [50], "everything after the first waits for an answer");
        // And what it shows is where the drag actually is, not the position
        // whose request happens to be in flight.
        compare(row.shownLevel, 65);

        row.level = 50;
        compare(row.asks, [50, 65], "the newest target goes, the ones passed through do not");
        compare(row.shownLevel, 65);

        row.level = 65;
        compare(row.asks, [50, 65]);
        compare(row.shownLevel, 65);
        row.destroy();
    }

    // A device that lands exactly where it already was publishes the reading
    // it published before, so no change ever arrives to release the next ask.
    function test_a_silent_answer_releases_the_row_on_its_own() {
        const row = rowComponent.createObject(null);
        verify(row);

        row.level = 40;
        row.ask(41);
        compare(row.asks, [41]);
        // The provider clamps back to 40, which is what it already published.
        tryCompare(row, "asked", -1, 5000);
        compare(row.shownLevel, 40);
        row.destroy();
    }

    function test_a_row_with_no_readable_level_asks_for_nothing() {
        const row = rowComponent.createObject(null);
        verify(row);
        row.known = false;

        row.ask(80);
        row.nudge(1);
        compare(row.asks, []);
        row.destroy();
    }

    // The wheel chains from what the row shows, so a second notch during a
    // slow answer moves on from the first rather than repeating it.
    function test_the_wheel_steps_from_what_the_row_shows() {
        const row = rowComponent.createObject(null);
        verify(row);
        row.step = 5;

        row.nudge(1);
        compare(row.shownLevel, 45);
        row.nudge(1);
        compare(row.shownLevel, 50);
        compare(row.asks, [45], "the second notch waits its turn");

        row.level = 45;
        compare(row.asks, [45, 50]);
        row.destroy();
    }

    // A notch asks for a level, not for an offset: a device left on 22 by
    // something else goes to 25 and to 20, instead of carrying that stray two
    // through every step it is ever given.
    function test_a_notch_lands_on_a_round_number() {
        const row = rowComponent.createObject(null);
        verify(row);
        row.step = 5;
        row.level = 22;

        row.nudge(1);
        compare(row.shownLevel, 25);
        row.destroy();

        const down = rowComponent.createObject(null);
        down.step = 5;
        down.level = 22;
        down.nudge(-1);
        compare(down.shownLevel, 20);

        // A level already on a multiple still moves a whole notch.
        down.level = 20;
        down.settled();
        down.nudge(-1);
        compare(down.shownLevel, 15);
        down.destroy();
    }
}
