import QtQuick
import QtTest
import "../qml" as Desktop

// The empty state, which is where the live session got stuck: clearing the
// history removed the only thing that was listening for Escape.
TestCase {
    id: testCase

    name: "ClipboardOverlay"

    QtObject {
        id: fakeSource

        property var providers: ({})
        property var sent: []

        function sendCommand(provider, verb, options) {
            fakeSource.sent.push({"verb": verb, "options": options});
            return 1;
        }
    }

    Desktop.ClipboardOverlay {
        id: overlay

        providerSource: fakeSource
        reducedMotion: false
    }

    function entries(count) {
        const list = [];
        for (let index = 0; index < count; ++index)
            list.push({"index": index, "preview": "entrada " + index});
        return list;
    }

    function init() {
        fakeSource.sent = [];
        fakeSource.providers = {"clipboard": {"entries": testCase.entries(3), "truncated": false}};
    }

    // Key delivery into a separate Window is not something an offscreen test
    // can drive, so these check the condition that actually broke: whether
    // anything inside the overlay is holding the keyboard at all. With the
    // history emptied, the list went invisible and took the focus — and every
    // key binding, Escape included — with it.
    function test_an_emptied_history_still_holds_the_keyboard() {
        // What `Vaciar` leaves behind: the provider still offers a history, it
        // simply has nothing in it.
        fakeSource.providers = {"clipboard": {"entries": [], "truncated": false}};
        compare(overlay.entries.length, 0);

        verify(overlay.activeFocusItem !== null,
               "an emptied overlay must still own the keyboard");
    }

    function test_a_populated_history_holds_the_keyboard_too() {
        compare(overlay.entries.length, 3);
        verify(overlay.activeFocusItem !== null);
    }

    function test_focus_survives_the_history_emptying_and_filling_again() {
        fakeSource.providers = {"clipboard": {"entries": [], "truncated": false}};
        verify(overlay.activeFocusItem !== null);

        fakeSource.providers = {"clipboard": {"entries": testCase.entries(2), "truncated": false}};
        verify(overlay.activeFocusItem !== null,
               "the list must take the keyboard back when there is one again");
    }

    function test_clearing_asks_the_provider_rather_than_emptying_locally() {
        overlay.clear();
        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].verb, "clear");
        // The list still shows what the provider last reported: the overlay
        // never paints the result it asked for.
        compare(overlay.entries.length, 3);
    }

    function test_removing_names_the_entry_it_removes() {
        overlay.remove(2);
        compare(fakeSource.sent[0].verb, "remove");
        compare(fakeSource.sent[0].options.index, 2);
    }
}
