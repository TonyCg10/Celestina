import QtQuick
import QtTest
import "../qml" as Desktop

// What the control centre shows, and what it refuses to show. Constructed
// offscreen: this proves the reading rules, never the appearance.
TestCase {
    id: testCase

    name: "ControlCentre"

    // A provider source that records what was asked for and answers with an
    // id, so a request's life can be driven from the test.
    QtObject {
        id: fakeSource

        property var providers: ({})
        property var sent: []
        property int nextId: 1

        signal commandResult(int requestId, string state, string reason)

        function sendCommand(provider, verb, options) {
            fakeSource.sent.push({"provider": provider, "verb": verb});
            return fakeSource.nextId++;
        }
    }

    Desktop.ControlCentre {
        id: centre

        providerSource: fakeSource
        reducedMotion: false
    }

    function init() {
        fakeSource.sent = [];
        fakeSource.nextId = 1;
        centre.outcomes = ({});
        centre.awaiting = ({});
        fakeSource.providers = {
            "audio": {"volume": 40, "muted": false},
            "night-light": {"active": false},
            "caffeine": {"active": false},
            "notifications": {"quiet": false, "unread": 0},
            "settings": {"levelStep": 5}
        };
    }

    function test_a_step_uses_the_step_the_person_chose() {
        fakeSource.providers = {"settings": {"levelStep": 10}};
        compare(centre.levelStep, 10);

        // And falls back to the panel's own step when settings say nothing.
        fakeSource.providers = {};
        compare(centre.levelStep, 5);
    }

    function test_a_request_is_pending_until_something_answers() {
        centre.send("audio", "mute-toggle", {});
        compare(centre.outcomeOf("mute-toggle").state, "pending");

        fakeSource.commandResult(1, "confirmed", "");
        compare(centre.outcomeOf("mute-toggle").state, "confirmed");
    }

    function test_a_failure_keeps_its_reason() {
        centre.send("night-light", "night-light-toggle", {});
        fakeSource.commandResult(1, "failed", "cannot start wlsunset");

        const outcome = centre.outcomeOf("night-light-toggle");
        compare(outcome.state, "failed");
        compare(outcome.reason, "cannot start wlsunset");
    }

    function test_an_answer_to_another_request_is_ignored() {
        centre.send("audio", "mute-toggle", {});
        // A result for an id this surface never asked about must not rewrite
        // any control's state.
        fakeSource.commandResult(99, "failed", "somebody else's problem");
        compare(centre.outcomeOf("mute-toggle").state, "pending");
    }

    function test_a_provider_that_is_not_there_reads_as_absent() {
        fakeSource.providers = {};
        verify(centre.nightLight === undefined);
        verify(centre.notifications === undefined);
        // Nothing is claimed to be off; the surface says there is no provider.
        verify(centre.audio === undefined);
    }
}
