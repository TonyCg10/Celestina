import QtQuick
import QtTest
import "../qml" as Desktop

// The selector is a view of compositor truth. Requests may become pending,
// but rows leave only when a later Melibea provider frame says they did.
TestCase {
    id: testCase

    name: "BubbleSelector"

    QtObject {
        id: fakeSource

        property int revision: 0
        property var providers: ({})
        property var sent: []
        property int nextId: 1

        signal changed()
        signal commandResult(int requestId, string state, string reason)

        function publish(windows) {
            providers = {
                "melibea": {
                    "available": true,
                    "revision": String(revision + 1),
                    "windows": windows
                }
            };
            revision += 1;
            changed();
        }

        function sendCommand(provider, verb, options) {
            sent.push({
                "provider": provider,
                "verb": verb,
                "options": options
            });
            return nextId++;
        }
    }

    Desktop.BubbleSelector {
        id: selector

        visible: true
        providerSource: fakeSource
        reducedMotion: true
    }

    // The selector above declares reduced motion, which is the right default for the
    // existing cases. Anchored travel needs one that animates, so it gets its own pair.
    QtObject {
        id: movingSource

        property int revision: 0
        property var providers: ({})
        property var sent: []
        property int nextId: 1

        signal changed()
        signal commandResult(int requestId, string state, string reason)

        function publish(windows) {
            providers = {
                "melibea": {
                    "available": true,
                    "revision": String(revision + 1),
                    "windows": windows
                }
            };
            revision += 1;
            changed();
        }

        function sendCommand(provider, verb, options) {
            sent.push({
                "provider": provider,
                "verb": verb,
                "options": options
            });
            return nextId++;
        }
    }

    Desktop.BubbleSelector {
        id: movingSelector

        // Deliberately not visible. These cases call the action methods directly, and a
        // second visible window competes for activation with the one whose key handling
        // the input cases exercise.
        visible: false
        providerSource: movingSource
        reducedMotion: false
    }

    SignalSpy {
        id: dismissals

        target: selector
        signalName: "dismissed"
    }

    function initialWindows() {
        return [
            {
                "id": "18446744073709551615",
                "appId": "org.example.Editor",
                "title": "<b>Proyecto literal</b>"
            },
            {
                "id": "42",
                "appId": "org.example.Terminal",
                "title": "Terminal"
            }
        ];
    }

    function init() {
        dismissals.clear();
        fakeSource.sent = [];
        fakeSource.nextId = 1;
        selector.pendingRequest = 0;
        selector.pendingWindowId = "";
        selector.pendingVerb = "";
        selector.errorText = "";
        selector.currentIndex = 0;
        selector.bubbleAnchorOutput = "";
        selector.bubbleAnchorRect = Qt.rect(0, 0, 0, 0);
        fakeSource.publish(initialWindows());

        // The animating selector needs the same reset: a request left pending by the
        // previous case makes `sendAction` return early, and every later assertion then
        // reads a command that was never sent.
        movingSource.sent = [];
        movingSource.nextId = 1;
        movingSelector.pendingRequest = 0;
        movingSelector.pendingWindowId = "";
        movingSelector.pendingVerb = "";
        movingSelector.errorText = "";
        movingSelector.currentIndex = 0;
        movingSelector.bubbleAnchorOutput = "";
        movingSelector.bubbleAnchorRect = Qt.rect(0, 0, 0, 0);
        movingSource.publish(initialWindows());
    }

    function test_restore_preserves_exact_identity_and_waits_for_state() {
        selector.restoreCurrent();

        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].provider, "melibea");
        compare(fakeSource.sent[0].verb, "restore");
        compare(fakeSource.sent[0].options.window_id,
                "18446744073709551615");
        compare(selector.rowCount, 2);

        fakeSource.commandResult(1, "accepted", "");
        compare(selector.rowCount, 2,
                "accepted is not compositor state confirmation");
        compare(selector.pendingWindowId, "18446744073709551615");

        fakeSource.publish(initialWindows());
        compare(selector.rowCount, 2);
        compare(selector.pendingWindowId, "18446744073709551615");

        fakeSource.publish([initialWindows()[1]]);
        compare(selector.rowCount, 1);
        compare(selector.pendingRequest, 0);
        compare(selector.pendingWindowId, "");
        compare(dismissals.count, 1,
                "the chooser retires only after compositor confirmation");
    }

    function test_same_length_replacement_also_settles_pending_action() {
        selector.restoreCurrent();
        fakeSource.publish([
            initialWindows()[1],
            {"id": "77", "appId": "org.example.Browser",
             "title": "Navegador"}
        ]);

        compare(selector.rowCount, 2);
        compare(selector.pendingRequest, 0);
        compare(selector.pendingWindowId, "");
    }

    function test_close_targets_the_selected_window() {
        selector.currentIndex = 1;
        selector.closeCurrent();

        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].verb, "close");
        compare(fakeSource.sent[0].options.window_id, "42");
        compare(selector.rowCount, 2);
    }

    function test_delete_uses_the_close_command_path() {
        const list = findChild(selector.contentItem,
                               "celestina-bubble-list");
        verify(list !== null);
        selector.requestActivate();
        tryCompare(selector, "active", true);
        list.forceActiveFocus(Qt.TabFocusReason);
        tryCompare(list, "activeFocus", true);
        selector.currentIndex = 1;

        keyClick(Qt.Key_Delete);

        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].verb, "close");
        compare(fakeSource.sent[0].options.window_id, "42");
        compare(selector.rowCount, 2);
    }

    function test_titles_are_always_plain_text() {
        const title = findChild(selector.contentItem,
                                "celestina-bubble-title-0");
        verify(title !== null);
        compare(title.text, "<b>Proyecto literal</b>");
        compare(title.textFormat, Text.PlainText);
    }

    function test_pointer_restore_uses_the_same_command_path() {
        const restore = findChild(selector.contentItem,
                                  "celestina-bubble-restore-1");
        verify(restore !== null);
        mouseClick(restore, restore.width / 2, restore.height / 2,
                   Qt.LeftButton);

        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].verb, "restore");
        compare(fakeSource.sent[0].options.window_id, "42");
    }

    function test_failed_request_keeps_the_row_and_shows_safe_copy() {
        selector.restoreCurrent();
        fakeSource.commandResult(1, "failed", "raw diagnostic");

        compare(selector.rowCount, 2);
        compare(selector.pendingRequest, 0);
        verify(selector.errorText.length > 0);
        verify(selector.errorText.indexOf("raw diagnostic") < 0);
    }

    function test_reduced_motion_asks_for_no_travel_at_all() {
        // Someone who asked for less movement is asking for none, not for movement to
        // somewhere else, so the anchor is deliberately ignored rather than sent.
        selector.bubbleAnchorOutput = "DP-1";
        selector.bubbleAnchorRect = Qt.rect(1874, 9, 22, 22);
        selector.restoreCurrent();

        compare(fakeSource.sent.length, 1);
        const options = fakeSource.sent[0].options;
        compare(options.transition, "disabled");
        compare(options.anchor_output, undefined,
                "reduced motion must carry no anchor");
        compare(options.window_id, "18446744073709551615");
    }

    function test_restore_travels_to_the_anchor_the_panel_handed_over() {
        movingSelector.bubbleAnchorOutput = "DP-1";
        movingSelector.bubbleAnchorRect = Qt.rect(1874, 9, 22, 22);

        movingSelector.restoreCurrent();

        compare(movingSource.sent.length, 1);
        const options = movingSource.sent[0].options;
        compare(movingSource.sent[0].verb, "restore");
        compare(options.transition, "anchored");
        compare(options.anchor_output, "DP-1");
        compare(options.anchor_x, 1874);
        compare(options.anchor_y, 9);
        compare(options.anchor_width, 22);
        compare(options.anchor_height, 22);
        // The window is still named exactly, at full width.
        compare(options.window_id, "18446744073709551615");
    }

    function test_a_missing_anchor_asks_for_ordinary_compositor_motion() {
        // No anchor is not the same as reduced motion: Niri should still animate, just its
        // own way. Sending `disabled` here would silently take movement away from someone
        // who never asked for that.
        movingSelector.bubbleAnchorOutput = "";
        movingSelector.bubbleAnchorRect = Qt.rect(0, 0, 0, 0);

        movingSelector.restoreCurrent();

        compare(movingSource.sent.length, 1);
        const options = movingSource.sent[0].options;
        compare(options.transition, undefined);
        compare(options.anchor_output, undefined);
        compare(options.window_id, "18446744073709551615");
    }

    function test_an_empty_anchor_rectangle_is_not_sent_as_a_destination() {
        movingSelector.bubbleAnchorOutput = "DP-1";
        movingSelector.bubbleAnchorRect = Qt.rect(1874, 9, 0, 22);

        movingSelector.restoreCurrent();

        compare(movingSource.sent[0].options.transition, undefined,
                "a collapsed slot is not a place to travel to");
    }

    function test_closing_never_carries_a_transition() {
        // A close has no destination: the window is not going to the bubble, it is going
        // away. Melibea refuses a hint on it, so the selector must not compose one.
        movingSelector.bubbleAnchorOutput = "DP-1";
        movingSelector.bubbleAnchorRect = Qt.rect(1874, 9, 22, 22);

        movingSelector.closeCurrent();

        compare(movingSource.sent.length, 1);
        compare(movingSource.sent[0].verb, "close");
        compare(movingSource.sent[0].options.transition, undefined);
        compare(movingSource.sent[0].options.anchor_output, undefined);
    }
}
