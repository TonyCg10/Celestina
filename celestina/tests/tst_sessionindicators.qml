import QtQuick
import QtTest
import "../qml" as Desktop

// The two indicators that open menus: when they are on the bar at all, what
// they say, and how they can be operated.
//
// The keyboard cases run in a window that accepts focus. The panel's own
// surface does not — `panelSpec` maps it `KeyboardInteractivityNone` so a bar
// can never steal focus from the window a person is working in — so what these
// prove is the control, not a Tab route that does not exist on a live session.
// That boundary is recorded in the plan rather than papered over here.
TestCase {
    id: testCase

    name: "SessionIndicators"
    when: windowShown
    visible: true
    width: 400
    height: 60

    property var requested: []

    Desktop.SessionStatus {
        id: status

        anchors.centerIn: parent
        network: undefined
        bluetooth: undefined
        power: undefined
        onIndicatorMenuRequested: (kind, globalX, globalY) => {
            testCase.requested.push(kind);
        }
    }

    function init() {
        testCase.requested = [];
        status.network = undefined;
        status.bluetooth = undefined;
        status.power = undefined;
    }

    function networkIndicator() {
        return findChild(status, "celestina-network-indicator");
    }

    function bluetoothIndicator() {
        return findChild(status, "celestina-bluetooth-indicator");
    }

    // A confirmed link reads as the link, exactly as it always has.
    function test_a_confirmed_link_is_named_on_the_bar() {
        status.network = {
            "kind": "wifi", "connection": "Tonys 1",
            "networksState": "fresh", "networks": []
        };

        const link = testCase.networkIndicator();
        verify(link.visible);
        compare(link.reading, "Tonys 1");
        compare(link.Accessible.name, qsTr("Conectado por %1 a %2").arg("wifi").arg("Tonys 1"));

        status.network = {"kind": "ethernet", "connection": "Cable 1"};
        compare(link.reading, qsTr("cable"));
    }

    // The defect this case exists for. With no default route the indicator used
    // to disappear — taking away the entry point to the menu that lists the
    // networks a person could reconnect to.
    function test_a_session_with_no_link_keeps_its_entry_point() {
        status.network = {
            "networksState": "fresh",
            "networks": [{"id": "9f1c-1", "name": "Tonys 1", "active": false}]
        };

        const link = testCase.networkIndicator();
        verify(link.visible);
        // Honest about there being no connection, and it does not claim Wi-Fi
        // merely because an inventory exists.
        compare(link.reading, qsTr("sin red"));
        compare(link.Accessible.name, qsTr("Sin conexión de red"));

        // And it still opens the menu, which is the entire point.
        waitForRendering(status);
        mouseClick(link);
        compare(testCase.requested, ["network"]);
    }

    // Every list state is a reading worth an entry point, and none of them is
    // the same as the provider having withdrawn.
    function test_every_inventory_state_keeps_the_indicator_reachable() {
        const states = ["pending", "held", "unavailable", "fresh"];
        for (let index = 0; index < states.length; ++index) {
            status.network = {"networksState": states[index], "networks": []};
            const link = testCase.networkIndicator();
            verify(link.visible, states[index]);
            compare(link.reading, qsTr("sin red"));
        }
    }

    // The declared policy for a provider that has withdrawn entirely: nothing
    // is published, so there is nothing truthful to offer and no entry point.
    function test_a_withdrawn_provider_leaves_no_indicator() {
        status.network = undefined;

        const link = testCase.networkIndicator();
        verify(!link.visible);
    }

    // Connected, then not, with saved networks still known: the entry point
    // must not go with the link.
    function test_losing_the_link_does_not_remove_the_entry_point() {
        status.network = {
            "kind": "wifi", "connection": "Tonys 1", "networksState": "fresh",
            "networks": [{"id": "9f1c-1", "name": "Tonys 1", "active": true}]
        };
        const link = testCase.networkIndicator();
        verify(link.visible);

        status.network = {
            "networksState": "held",
            "networks": [{"id": "9f1c-1", "name": "Tonys 1", "active": false}]
        };
        verify(link.visible);
        compare(link.reading, qsTr("sin red"));
    }

    function test_the_bluetooth_indicator_keeps_its_settled_policy() {
        status.bluetooth = {"adapter": "on", "count": 0};
        const radio = testCase.bluetoothIndicator();
        verify(radio.visible);
        compare(radio.reading, qsTr("bt"));

        status.bluetooth = {"adapter": "off", "count": 0};
        verify(radio.visible);

        // A machine with no controller, and a provider that withdrew, both
        // leave the bar alone.
        status.bluetooth = {"adapter": "absent", "count": 0};
        verify(!radio.visible);
        status.bluetooth = undefined;
        verify(!radio.visible);
    }

    // A click opens the menu.
    function test_a_click_opens_each_menu() {
        status.network = {"networksState": "fresh", "networks": []};
        status.bluetooth = {"adapter": "on", "count": 0};

        waitForRendering(status);
        mouseClick(testCase.networkIndicator());
        mouseClick(testCase.bluetoothIndicator());
        compare(testCase.requested, ["network", "bluetooth"]);
    }

    // And so does Enter, and so does Space — really pressed, on a control that
    // really has the focus.
    function test_enter_and_space_open_the_menu() {
        status.network = {"networksState": "fresh", "networks": []};
        const link = testCase.networkIndicator();

        link.forceActiveFocus();
        verify(link.activeFocus);

        keyClick(Qt.Key_Return);
        compare(testCase.requested, ["network"]);

        keyClick(Qt.Key_Space);
        compare(testCase.requested, ["network", "network"]);

        keyClick(Qt.Key_Enter);
        compare(testCase.requested.length, 3);
    }

    // Focus that came from the keyboard is shown; focus that came from a click
    // is not. That is what `visualFocus` means, and the ring follows it rather
    // than following `activeFocus`.
    function test_the_focus_ring_follows_visual_focus_only() {
        status.network = {"networksState": "fresh", "networks": []};
        // Somewhere else for the focus to go, laid out before it is asked to
        // take it: an item that is not visible yet cannot.
        status.bluetooth = {"adapter": "on", "count": 0};
        waitForRendering(status);

        const link = testCase.networkIndicator();
        const ring = findChild(link, "celestina-indicator-focus");
        verify(ring);

        // Focus starts elsewhere, so each reason below is really applied
        // rather than being a no-op on an item that already had it.
        testCase.bluetoothIndicator().forceActiveFocus(Qt.TabFocusReason);

        // Reached by the keyboard: focused, and shown to be.
        link.forceActiveFocus(Qt.TabFocusReason);
        verify(link.activeFocus);
        verify(link.visualFocus);
        compare(ring.shown, link.visualFocus);

        // Reached by a click: focused, and not shown to be. Focus moves away
        // first so the reason is really re-applied rather than left over.
        testCase.bluetoothIndicator().forceActiveFocus(Qt.TabFocusReason);
        verify(!link.activeFocus);
        link.forceActiveFocus(Qt.MouseFocusReason);
        verify(link.activeFocus);
        verify(!link.visualFocus);
        // The ring follows visual focus, not the plain kind — which is the
        // whole distinction, and the reason it is not bound to `activeFocus`.
        compare(ring.shown, link.visualFocus);
    }
}
