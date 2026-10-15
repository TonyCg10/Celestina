import QtQuick
import QtTest
import "../qml" as Desktop

// What must survive a provider frame while somebody is touching a row.
//
// The audio provider publishes after every command it carries out, and again
// on its own poll. A row rebuilt by one of those frames takes the pointer
// grab, the drag it was in the middle of and everything it had asked for down
// with it — which is what made a dragged slider jump back to an older reading.
TestCase {
    id: testCase

    name: "AudioMenu"

    QtObject {
        id: fakeSource

        property var providers: ({})
        property int revision: 0
        property var sent: []

        function publish(next) {
            fakeSource.providers = next;
            fakeSource.revision = fakeSource.revision + 1;
            fakeSource.changed();
        }

        signal changed()

        function sendCommand(provider, verb, options) {
            fakeSource.sent.push({"provider": provider, "verb": verb,
                                  "options": options});
            return 1;
        }
    }

    Desktop.AudioMenu {
        id: menu

        providerSource: fakeSource
        outputName: "test-output"
        reducedMotion: true
    }

    function frame(appVolume) {
        return {
            "audio": {
                "volume": 75,
                "muted": false,
                "outputs": [{"id": 1, "name": "Altavoces", "default": true}],
                "inputs": [],
                "playbackApps": [{"id": 40, "name": "HOI4",
                                  "volume": appVolume, "muted": false}],
                "captureApps": []
            }
        };
    }

    function init() {
        fakeSource.sent = [];
        fakeSource.publish(testCase.frame(33));
    }

    function appRow() {
        return findChild(menu.contentItem, "celestina-level-row-40");
    }

    // The identity of the row, not just its numbers: an object that is
    // destroyed and rebuilt has no drag left to continue.
    function test_a_frame_does_not_rebuild_the_row_being_touched() {
        const row = testCase.appRow();
        verify(row);
        compare(row.level, 33);

        fakeSource.publish(testCase.frame(48));

        compare(testCase.appRow(), row, "the same row answered the new frame");
        compare(row.level, 48, "and it carries the new reading");
    }

    // The whole point of keeping it: what the row was asked for outlives the
    // frames that arrive while the request is still travelling.
    function test_a_frame_does_not_lose_what_the_row_asked_for() {
        const row = testCase.appRow();
        verify(row);

        row.ask(70);
        compare(row.shownLevel, 70);
        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].verb, "node-volume");
        compare(fakeSource.sent[0].options.id, 40);
        compare(fakeSource.sent[0].options.percent, 70);

        // A frame published before the change landed — the provider's poll, or
        // the read-back of an earlier request.
        fakeSource.publish(testCase.frame(33));

        compare(testCase.appRow(), row);
        compare(row.shownLevel, 70, "the older reading does not move the thumb");
    }
}
