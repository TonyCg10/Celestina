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

    QtObject {
        id: fakeLedger

        property int revision: 0

        function send(provider, verb, options, target, policy) {
            fakeSource.sent.push({
                "provider": provider,
                "verb": verb,
                "options": options,
                "target": target,
                "policy": policy
            });
            return fakeSource.nextId++;
        }

        function stateOf(provider, verb) {
            return {};
        }

        function isPending(provider, verb) {
            return false;
        }
    }

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
        property var requests: fakeLedger

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

    Component {
        id: animatedCentreComponent

        Desktop.ControlCentre {
            providerSource: fakeSource
            reducedMotion: false
        }
    }

    function init() {
        fakeSource.sent = [];
        fakeSource.nextId = 1;
        fakeSource.providers = {
            "audio": {"volume": 40, "muted": false},
            "night-light": {"active": false},
            "caffeine": {"active": false},
            "notifications": {"quiet": false, "unread": 0},
            "power": {"active": "performance", "count": 3},
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

    function test_a_reported_option_cross_fades_instead_of_cutting_text() {
        const animatedCentre = animatedCentreComponent.createObject(null);
        verify(animatedCentre);
        const reading = findChild(
            animatedCentre.contentItem,
            "celestina-control-reading-night-light-night-light-toggle");
        verify(reading);
        compare(reading.displayedValue, qsTr("apagada"));
        compare(reading.outgoingValue, "");
        compare(reading.progress, 1.0);

        const nextProviders = {
            "audio": {"volume": 40, "muted": false},
            "night-light": {"active": true},
            "caffeine": {"active": false},
            "notifications": {"quiet": false, "unread": 0},
            "power": {"active": "performance", "count": 3},
            "settings": {"levelStep": 5}
        };
        fakeSource.publish(nextProviders);

        tryCompare(reading, "displayedValue", qsTr("encendida"));
        compare(reading.outgoingValue, qsTr("apagada"));
        verify(reading.progress < 1.0,
               "the provider commit retains the old reading for the fade");
        wait(40);
        verify(reading.progress > 0.0 && reading.progress < 1.0,
               "both reading layers remain in the motionFast transition");
        tryCompare(reading, "progress", 1.0);
        compare(reading.outgoingValue, "");

        // Reduced motion removes the duration, not the provider-owned state
        // change or the final value.
        animatedCentre.reducedMotion = true;
        const reducedProviders = {
            "audio": {"volume": 40, "muted": false},
            "night-light": {"active": false},
            "caffeine": {"active": false},
            "notifications": {"quiet": false, "unread": 0},
            "power": {"active": "performance", "count": 3},
            "settings": {"levelStep": 5}
        };
        fakeSource.publish(reducedProviders);
        tryCompare(reading, "displayedValue", qsTr("apagada"));
        compare(reading.progress, 1.0);
        compare(reading.outgoingValue, "");
        animatedCentre.destroy();
    }

    function test_night_light_switch_waits_for_provider_confirmation() {
        const nightSwitch = findChild(
            centre.contentItem,
            "celestina-night-light-switch");
        verify(nightSwitch);
        compare(nightSwitch.checked, false);

        let checkedChanges = 0;
        const countCheckedChange = function() {
            checkedChanges += 1;
        };
        nightSwitch.checkedChanged.connect(countCheckedChange);

        // `click()` is the AbstractButton activation path used by keyboard and
        // accessibility too: it must request once without painting acceptance.
        nightSwitch.click();
        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].provider, "night-light");
        compare(fakeSource.sent[0].verb, "night-light-toggle");
        compare(nightSwitch.checked, false);
        compare(checkedChanges, 0);

        const confirmedProviders = {
            "audio": {"volume": 40, "muted": false},
            "night-light": {"active": true},
            "caffeine": {"active": false},
            "notifications": {"quiet": false, "unread": 0},
            "power": {"active": "performance", "count": 3},
            "settings": {"levelStep": 5}
        };
        fakeSource.publish(confirmedProviders);

        tryCompare(nightSwitch, "checked", true);
        compare(checkedChanges, 1);
        nightSwitch.checkedChanged.disconnect(countCheckedChange);
    }





    function findByHelpText(item, helpText) {
        if (item.helpText !== undefined && item.helpText === helpText)
            return item;

        if (item.children === undefined)
            return null;

        for (let index = 0; index < item.children.length; ++index) {
            const found = testCase.findByHelpText(item.children[index], helpText);
            if (found)
                return found;
        }

        return null;
    }

    function test_power_stays_in_the_control_centre_and_cycles_through_its_ledger() {
        compare(centre.power.active, "performance");

        const button = testCase.findByHelpText(
            centre.contentItem,
            qsTr("Cambiar al siguiente perfil que ofrece el demonio"));
        verify(button, "the power profile action remains in ControlCentre");
        button.click();

        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].provider, "power");
        compare(fakeSource.sent[0].verb, "cycle");
        compare(fakeSource.sent[0].target, "cycle");
        compare(fakeSource.sent[0].policy, "immediate");
    }

    function test_a_provider_that_is_not_there_reads_as_absent() {
        fakeSource.providers = {};
        verify(centre.nightLight === undefined);
        verify(centre.notifications === undefined);
        // Nothing is claimed to be off; the surface says there is no provider.
        verify(centre.audio === undefined);
    }
}
