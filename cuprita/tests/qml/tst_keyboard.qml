import QtQuick
import QtTest 1.3
import org.celestina.cuprita 1.0
import "../../qml/components"
import "../../qml/pages"
import "fakes"

// The keyboard pass: the strip is one Tab stop, Tab then reaches the page,
// the arrows walk a list and Enter runs the current row's primary action; the
// rows read kind, name and state to a screen reader.
TestCase {
    id: testCase
    name: "Keyboard"
    width: 640
    height: 560
    visible: true
    when: windowShown

    Component {
        id: windowComponent

        Item {
            id: host
            property alias strip: navStrip
            property alias page: bluetoothPage
            property alias fake: controller
            width: 640
            height: 560

            FakeBluetoothController { id: controller }

            NavStrip {
                id: navStrip
                model: Sections.all
            }

            BluetoothPage {
                id: bluetoothPage
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: navStrip.bottom
                anchors.bottom: parent.bottom
                controller: controller
                devices: controller.devices
                backdropSource: host
            }
        }
    }

    function within(item, ancestor) {
        for (let node = item; node; node = node.parent)
            if (node === ancestor)
                return true
        return false
    }

    function test_the_strip_is_one_tab_stop() {
        const host = createTemporaryObject(windowComponent, testCase)
        host.strip.forceActiveFocus()
        tryVerify(function() { return within(testCase.Window.activeFocusItem, host.strip) })
        keyClick(Qt.Key_Tab)
        // One Tab leaves the strip for the page.
        verify(!within(testCase.Window.activeFocusItem, host.strip))
        verify(within(testCase.Window.activeFocusItem, host.page))
    }

    function test_enter_runs_the_first_rows_primary_action() {
        const host = createTemporaryObject(windowComponent, testCase)
        const list = findChild(host.page, "deviceList")
        tryCompare(list, "count", 2)
        list.forceActiveFocus()
        list.currentIndex = 0
        tryCompare(list, "activeFocus", true)
        keyClick(Qt.Key_Return)
        // The headphones are paired and disconnected: Enter connects them.
        compare(host.fake.calls, ["connect:AA:BB:CC:00:00:01"])
        // Down walks to the phone, a stranger: Enter pairs it.
        keyClick(Qt.Key_Down)
        compare(list.currentIndex, 1)
        keyClick(Qt.Key_Return)
        compare(host.fake.calls, ["connect:AA:BB:CC:00:00:01", "pair:AA:BB:CC:00:00:02"])
    }

    function test_a_row_reads_kind_name_state_and_battery() {
        const host = createTemporaryObject(windowComponent, testCase)
        const list = findChild(host.page, "deviceList")
        tryCompare(list, "count", 2)
        const row = list.itemAtIndex(0)
        compare(row.Accessible.role, Accessible.ListItem)
        compare(row.Accessible.name, qsTr("Dispositivo de audio, Headphones, Emparejado, batería al 80 %"))
        compare(findChild(host.page, "settingSwitch").Accessible.role, Accessible.CheckBox)
    }

    function test_menu_key_opens_the_rows_menu() {
        const host = createTemporaryObject(windowComponent, testCase)
        const list = findChild(host.page, "deviceList")
        tryCompare(list, "count", 2)
        list.forceActiveFocus()
        list.currentIndex = 0
        tryCompare(list, "activeFocus", true)
        keyClick(Qt.Key_F10, Qt.ShiftModifier)
        const menu = findChild(host.page, "deviceMenu")
        tryCompare(menu, "opened", true)
        keyClick(Qt.Key_Escape)
        tryCompare(menu, "visible", false)
        list.forceActiveFocus()
        tryCompare(list, "activeFocus", true)
        keyClick(Qt.Key_Menu)
        tryCompare(menu, "opened", true)
    }

    Component {
        id: sliderComponent

        VolumeSlider {
            property var sent: []
            width: 300
            height: 32
            value: 50
            onRequested: function(v) { sent = sent.concat([v]) }
        }
    }

    function test_arrows_step_a_volume_slider() {
        const slider = createTemporaryObject(sliderComponent, testCase)
        slider.forceActiveFocus()
        tryCompare(slider, "activeFocus", true)
        verify(slider.activeFocusOnTab)
        keyClick(Qt.Key_Right)
        tryCompare(slider, "sent", [51])
        wait(CelestinaTheme.motionFast * 2)
        slider.value = 51
        keyClick(Qt.Key_Right, Qt.ShiftModifier)
        // Exactly one write, of the 5 % step.
        tryCompare(slider, "sent", [51, 56])
        wait(CelestinaTheme.motionFast * 3)
        compare(slider.sent, [51, 56])
    }
}
