import QtQuick
import QtTest
import "../qml" as Desktop

// What it takes to end a session here, and what is shown about it afterwards.
TestCase {
    id: testCase

    name: "SessionMenu"

    QtObject {
        id: fakeShell

        property var sent: []

        signal commandOutcome(string verb, string state, string reason)

        function send(verb) {
            fakeShell.sent.push(verb);
        }
    }

    Desktop.SessionMenu {
        id: menu

        shellSource: fakeShell
        reducedMotion: false
    }

    function init() {
        fakeShell.sent = [];
        menu.armed = "";
        menu.outcomeVerb = "";
        menu.outcomeState = "";
        menu.outcomeReason = "";
    }

    function test_nothing_irreversible_happens_on_one_press() {
        menu.press("power-off");
        compare(fakeShell.sent.length, 0);
        compare(menu.armed, "power-off");
    }

    function test_the_second_press_is_the_one_that_asks() {
        menu.press("power-off");
        menu.press("power-off");
        compare(fakeShell.sent, ["power-off"]);
        // Arming does not survive the request it produced.
        compare(menu.armed, "");
        compare(menu.outcomeState, "pending");
    }

    function test_arming_another_action_disarms_the_first() {
        menu.press("power-off");
        menu.press("reboot");
        compare(fakeShell.sent.length, 0);
        compare(menu.armed, "reboot");
    }

    function test_a_refusal_is_shown_with_its_reason() {
        menu.press("suspend");
        menu.press("suspend");
        fakeShell.commandOutcome("suspend", "failed", "no locker provider");
        compare(menu.outcomeState, "failed");
        compare(menu.outcomeReason, "no locker provider");
    }

    function test_an_outcome_for_another_verb_is_ignored() {
        menu.press("reboot");
        menu.press("reboot");
        fakeShell.commandOutcome("power-off", "confirmed", "");
        compare(menu.outcomeVerb, "reboot");
        compare(menu.outcomeState, "pending");
    }
}
