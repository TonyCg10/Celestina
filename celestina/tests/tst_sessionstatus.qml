import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle
import "../qml" as Desktop

// The live failure: Bluetooth was powered, nothing was paired to it, and the
// panel showed nothing at all. The provider withdrew itself whenever the
// connected list was empty, so "on and idle" and "not there" were the same
// silence. Four states arrive here now and this is what each one looks like.
//
// The row sits in a shown window because `visible` is effective visibility: an
// item whose own binding is true still reads false while nothing above it is on
// screen, and it is the binding this case is about.
TestCase {
    id: testCase

    name: "SessionStatus"
    when: windowShown

    Desktop.BackdropInk {
        id: testInk
    }

    function on(count, first) {
        const reading = {"adapter": "on", "count": count};
        if (first !== undefined)
            reading.first = first;

        return reading;
    }

    Window {
        id: host

        width: 400
        height: 40
        visible: true

        Desktop.SessionStatus {
            id: status

            ink: testInk
            anchors.verticalCenter: parent.verticalCenter
            network: undefined
            bluetooth: undefined
        }
    }

    function radio() {
        return findChild(status, "celestina-bluetooth-indicator");
    }

    function test_a_powered_adapter_with_nothing_on_it_stays_visible() {
        status.bluetooth = testCase.on(0);

        compare(radio().text, "bt");
        compare(radio().visible, true);
        verify(radio().Accessible.name.indexOf("sin dispositivos") >= 0);
    }

    function test_a_connected_device_is_counted_and_named() {
        status.bluetooth = testCase.on(2, "S25 Ultra de Antonio");

        compare(radio().text, "bt 2");
        compare(radio().visible, true);
        verify(radio().Accessible.name.indexOf("S25 Ultra de Antonio") >= 0);
    }

    function test_an_adapter_that_is_off_says_so_rather_than_disappearing() {
        status.bluetooth = {"adapter": "off", "count": 0};

        compare(radio().text, "bt apagado");
        compare(radio().visible, true);
    }

    function test_a_machine_with_no_adapter_shows_nothing() {
        status.bluetooth = {"adapter": "absent", "count": 0};

        compare(radio().visible, false);
    }

    // An unreadable query withdraws the provider rather than guessing, and the
    // panel must not invent a state for what nobody could read.
    function test_an_unread_adapter_shows_nothing() {
        status.bluetooth = undefined;

        compare(radio().visible, false);
        compare(radio().Accessible.name, "");
    }

    function test_the_row_contains_only_the_two_connectivity_controls() {
        compare(status.children.length, 2);
        verify(findChild(status, "celestina-network-indicator"));
        verify(findChild(status, "celestina-bluetooth-indicator"));
    }
}
