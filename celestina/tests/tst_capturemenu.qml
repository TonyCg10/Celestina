import QtQuick
import QtTest
import "../qml" as Desktop

// The toolbox keeps the real Menu lifecycle: ordered semantic rows, one
// actionable capture request and normal dismissal after activation. Recording
// is the row with state behind it, and every claim it makes is the provider's.
TestCase {
    id: testCase

    name: "CaptureMenu"

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

    Desktop.CaptureMenu {
        id: menu

        providerSource: fakeSource
        outputName: "test-output"
        reducedMotion: true
    }

    SignalSpy {
        id: captureSpy

        target: menu
        signalName: "captureRequested"
    }

    SignalSpy {
        id: recordSpy

        target: menu
        signalName: "recordRequested"
    }

    SignalSpy {
        id: dismissedSpy

        target: menu
        signalName: "dismissed"
    }

    function init() {
        captureSpy.clear();
        recordSpy.clear();
        dismissedSpy.clear();
        fakeSource.sent = [];
        fakeSource.publish({
            "recorder": {"available": true, "recording": false}
        });
        if (!menu.menu.visible)
            menu.menu.open();
    }

    function rowAt(index) {
        return menu.menu.itemAt(index);
    }

    function test_it_keeps_an_extensible_ordered_hierarchy() {
        compare(menu.entries.length, 4);
        compare(menu.entries[0].kind, "header");
        compare(menu.entries[1].kind, "section");
        compare(menu.entries[2].kind, "capture");
        compare(menu.entries[3].kind, "record");
        compare(menu.menu.count, 4);

        const header = testCase.rowAt(0);
        const section = testCase.rowAt(1);
        const action = testCase.rowAt(2);
        verify(header.header);
        verify(!header.actionable);
        compare(header.text, qsTr("Caja de herramientas"));
        compare(header.iconName, "toolbox");
        verify(section.sectionLabel);
        verify(!section.actionable);
        verify(action.actionable);
        compare(action.text, qsTr("Capturar pantalla"));
        compare(action.iconName, "scissors");
    }

    // Which screen is a question this menu cannot answer: it is open on one
    // output, and the bug may well be on another. So it raises the question
    // and starts nothing itself.
    function test_starting_a_recording_asks_which_screen() {
        const record = testCase.rowAt(3);
        verify(record.actionable);
        compare(record.iconName, "film");
        compare(record.text, qsTr("Grabar una pantalla…"));

        record.triggered();

        compare(recordSpy.count, 1);
        compare(fakeSource.sent.length, 0, "the menu starts no recording of its own");
        compare(menu.recording, false);
        tryCompare(menu.menu, "visible", false);
    }

    // Stopping needs no question, so it is said straight to the provider —
    // which is also the only thing that can say the session is recording.
    function test_stopping_goes_straight_to_the_provider() {
        fakeSource.publish({
            "recorder": {
                "available": true,
                "recording": true,
                "output": "test-output",
                "since": Date.now() - 65000
            }
        });
        compare(menu.recording, true);
        // The elapsed time is measured from the provider's instant, not from
        // when this menu was opened.
        compare(menu.elapsedText(), "1:05");
        compare(testCase.rowAt(3).text,
                qsTr("Detener la grabación · %1").arg("1:05"));

        testCase.rowAt(3).triggered();

        compare(recordSpy.count, 0, "stopping asks nobody which screen");
        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].provider, "recorder");
        compare(fakeSource.sent[0].verb, "record-stop");
        // And nothing is painted from this click: the row still says the
        // session is recording until the provider says otherwise.
        compare(menu.recording, true);
        tryCompare(menu.menu, "visible", false);
    }

    // The clock ticks once a second. If it moved through the menu's model,
    // every row of the real Menu would be destroyed and rebuilt with it.
    function test_the_running_clock_moves_no_row() {
        fakeSource.publish({
            "recorder": {
                "available": true,
                "recording": true,
                "output": "test-output",
                "since": Date.now() - 5000
            }
        });
        const rowsBefore = [testCase.rowAt(0), testCase.rowAt(1),
                            testCase.rowAt(2), testCase.rowAt(3)];
        const textBefore = rowsBefore[3].text;

        menu.nowMs = Date.now() + 60000;

        compare(testCase.rowAt(0), rowsBefore[0]);
        compare(testCase.rowAt(1), rowsBefore[1]);
        compare(testCase.rowAt(2), rowsBefore[2]);
        compare(testCase.rowAt(3), rowsBefore[3], "the row itself is not rebuilt");
        verify(rowsBefore[3].text !== textBefore, "and its label still moved");
    }

    // A recording that would not start says so where it was asked for: by the
    // time the helper knows, the menu that asked has closed itself.
    function test_a_failed_start_is_said_in_the_row() {
        fakeSource.publish({
            "recorder": {"available": true, "recording": false,
                         "failure": "start-failed"}
        });

        compare(testCase.rowAt(3).text,
                qsTr("Grabar una pantalla… · %1")
                .arg(qsTr("no se pudo empezar a grabar")));
        verify(testCase.rowAt(3).actionable, "and it can still be tried again");
    }

    // A session without the recording tool says so in the row itself rather
    // than offering something it cannot do, or hiding it with no explanation.
    function test_a_session_with_no_recorder_says_so_in_the_row() {
        fakeSource.publish({"recorder": {"available": false, "recording": false}});

        const record = testCase.rowAt(3);
        verify(!record.actionable);
        compare(record.text, qsTr("Grabar pantalla · sin grabador"));

        record.triggered();
        compare(fakeSource.sent.length, 0);
        compare(recordSpy.count, 0);
    }

    function test_the_action_requests_once_and_closes_the_real_menu() {
        const action = testCase.rowAt(2);
        verify(action !== null);

        action.triggered();

        compare(captureSpy.count, 1);
        tryCompare(menu.menu, "visible", false);
        tryCompare(dismissedSpy, "count", 1);
    }
}
