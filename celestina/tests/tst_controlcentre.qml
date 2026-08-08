import QtQuick
import QtTest
import "../qml" as Desktop

// The life of a request is not tested here any more: it belongs to the durable
// ledger on the bridge, and a fake source cannot stand in for it. The control
// centre's own contract — `accepted` finishes an immediate verb — is driven
// against the real ledger in `indicatormenu_test.cpp`.

// What the control centre shows, and what it refuses to show. Constructed
// offscreen: this proves the reading rules, never the appearance.
TestCase {
    id: testCase

    name: "ControlCentre"

    // A provider source that records what was asked for and answers with an
    // id, so a request's life can be driven from the test.
    QtObject {
        id: fakeSource

        // The real bridge reports whether the helper is there at all, and says
        // so through the same signal every frame arrives on.
        property bool available: true
        property var providers: ({})
        // The host's snapshot revision. Every surface reads it before a
        // provider key so that a key inserted into an existing map still
        // re-evaluates the binding; the fake carries it for the same reason.
        property int revision: 0
        property var sent: []
        property int nextId: 1

        // Publishes a provider set the way a helper frame does: one revision
        // per frame, whether or not the set of keys changed.
        function publish(next) {
            fakeSource.providers = next;
            fakeSource.revision = fakeSource.revision + 1;
            fakeSource.changed();
        }

        signal changed()

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
        fakeSource.providers = {
            "audio": {"volume": 40, "muted": false},
            "night-light": {"active": false},
            "caffeine": {"active": false},
            "notifications": {"quiet": false, "unread": 0},
            "settings": {"levelStep": 5}
        };
    }

    // A provider the helper had nothing to say about yet is inserted into a
    // later frame of the same generation. The centre is already open and bound
    // by then, so a binding that read the key directly never noticed it — the
    // shape of the recorded media failure, and `weather` is the reading that
    // really does arrive minutes late.
    function test_a_provider_inserted_after_binding_becomes_visible() {
        verify(centre.weather === undefined);

        const withWeather = fakeSource.providers;
        withWeather["weather"] = {"summary": "Despejado", "temperature": 21};
        fakeSource.publish(withWeather);

        verify(centre.weather !== undefined);
        compare(centre.weather.temperature, 21);

        // And a provider that goes away is gone, not remembered.
        const withoutWeather = fakeSource.providers;
        delete withoutWeather["weather"];
        fakeSource.publish(withoutWeather);
        verify(centre.weather === undefined);
    }

    function test_a_step_uses_the_step_the_person_chose() {
        fakeSource.providers = {"settings": {"levelStep": 10}};
        compare(centre.levelStep, 10);

        // And falls back to the panel's own step when settings say nothing.
        fakeSource.providers = {};
        compare(centre.levelStep, 5);
    }





    function test_a_provider_that_is_not_there_reads_as_absent() {
        fakeSource.providers = {};
        verify(centre.nightLight === undefined);
        verify(centre.notifications === undefined);
        // Nothing is claimed to be off; the surface says there is no provider.
        verify(centre.audio === undefined);
    }
}
