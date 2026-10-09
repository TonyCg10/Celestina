import QtQuick
import QtTest 1.3
import org.celestina.cuprita 1.0
import "../../qml/pages"
import "fakes"

// The Bluetooth page over the scripted devices: two rows, the headphones show
// their battery, their connect action connects them and their menu forgets
// them; the search spins while the adapter searches; and what the pairing
// agent asks opens the pairing dialog, whose answers reach the controller.
TestCase {
    id: testCase
    name: "BluetoothPage"
    width: 640
    height: 480
    visible: true
    when: windowShown

    Component {
        id: pageComponent

        Item {
            id: host
            property alias page: bluetoothPage
            property alias fake: controller
            width: 640
            height: 480

            FakeBluetoothController { id: controller }

            BluetoothPage {
                id: bluetoothPage
                anchors.fill: parent
                controller: controller
                devices: controller.devices
                backdropSource: host
            }
        }
    }

    function headphones(host) {
        const list = findChild(host.page, "deviceList")
        tryCompare(list, "count", 2)
        return list.itemAtIndex(0)
    }

    function test_two_rows_and_the_battery() {
        const host = createTemporaryObject(pageComponent, testCase)
        const row = headphones(host)
        compare(row.model.name, "Headphones")
        compare(findChild(row, "deviceBattery").text, "80 %")
    }

    function test_connect_connects_the_headphones() {
        const host = createTemporaryObject(pageComponent, testCase)
        const row = headphones(host)
        const join = findChild(row, "joinButton")
        compare(join.Accessible.name, "Conectar")
        waitForRendering(host.page)
        mouseClick(join)
        compare(host.fake.calls, ["connect:AA:BB:CC:00:00:01"])
        tryCompare(findChild(row, "deviceState"), "text", "Conectado")
    }

    function test_the_menu_forgets_the_headphones() {
        const host = createTemporaryObject(pageComponent, testCase)
        const row = headphones(host)
        // A stranger has no menu: there is nothing to forget.
        const list = findChild(host.page, "deviceList")
        tryVerify(function() { return list.itemAtIndex(1) !== null })
        verify(!findChild(list.itemAtIndex(1), "menuButton").visible)
        waitForRendering(host.page)
        mouseClick(findChild(row, "menuButton"))
        const menu = findChild(host.page, "deviceMenu")
        tryCompare(menu, "opened", true)
        const forget = findChild(menu.contentItem, "forgetItem")
        verify(forget)
        compare(forget.text, "Olvidar")
        waitForRendering(host.page)
        mouseClick(forget)
        compare(host.fake.calls, ["forget:AA:BB:CC:00:00:01"])
        // Closed, so its pointer shield is gone with it.
        tryCompare(menu, "visible", false)
        tryCompare(list, "count", 1)
    }

    function test_the_search_spins_while_discovering() {
        const host = createTemporaryObject(pageComponent, testCase)
        headphones(host)
        const spinner = findChild(host.page, "searchSpinner")
        const search = findChild(host.page, "searchButton")
        verify(!spinner.visible)
        compare(search.text, "Buscar")
        waitForRendering(host.page)
        mouseClick(search)
        compare(host.fake.calls, ["setDiscovering:true"])
        verify(spinner.visible)
        compare(search.text, qsTr("Detener búsqueda"))
    }

    function test_a_passkey_to_confirm_shows_and_matches() {
        const host = createTemporaryObject(pageComponent, testCase)
        headphones(host)
        const dialog = findChild(host.page, "pairingDialog")
        host.fake.agentRequest("confirm", "Auriculares", 123456)
        tryCompare(dialog, "shown", true)
        compare(findChild(dialog, "passkeyText").text, "123456")
        const confirm = findChild(dialog, "confirmButton")
        compare(confirm.text, "Coincide")
        verify(findChild(dialog, "rejectButton").visible)
        waitForRendering(host.page)
        mouseClick(confirm)
        compare(host.fake.calls, ["answerAgent:confirm:yes"])
        tryCompare(dialog, "shown", false)
    }

    function test_escape_cancels_the_pairing() {
        const host = createTemporaryObject(pageComponent, testCase)
        headphones(host)
        const dialog = findChild(host.page, "pairingDialog")
        host.fake.agentRequest("confirm", "Auriculares", 123456)
        tryCompare(dialog, "shown", true)
        tryCompare(findChild(dialog, "confirmButton"), "activeFocus", true)
        keyClick(Qt.Key_Escape)
        compare(host.fake.calls, ["answerAgent:confirm:cancel"])
        tryCompare(dialog, "shown", false)
    }

    function test_a_pin_is_typed_and_sent_once() {
        const host = createTemporaryObject(pageComponent, testCase)
        headphones(host)
        const dialog = findChild(host.page, "pairingDialog")
        host.fake.agentRequest("pin", "Teclado", 0)
        tryCompare(dialog, "shown", true)
        const field = findChild(dialog, "pinField")
        tryCompare(field, "activeFocus", true)
        verify(!findChild(dialog, "confirmButton").enabled)
        keyClick(Qt.Key_0)
        keyClick(Qt.Key_4)
        keyClick(Qt.Key_2)
        keyClick(Qt.Key_7)
        keyClick(Qt.Key_Return)
        compare(host.fake.calls, ["answerAgent:pin:0427"])
        tryCompare(dialog, "shown", false)
        compare(field.text, "")
    }

    function test_a_shown_passkey_closes_when_the_agent_says_so() {
        const host = createTemporaryObject(pageComponent, testCase)
        headphones(host)
        const dialog = findChild(host.page, "pairingDialog")
        host.fake.agentRequest("display", "Teclado", 42)
        tryCompare(dialog, "shown", true)
        compare(findChild(dialog, "passkeyText").text, "000042")
        verify(!findChild(dialog, "confirmButton").visible)
        host.fake.agentClosed()
        tryCompare(dialog, "shown", false)
        compare(host.fake.calls, [])
    }

    function test_airplane_mode_disables_the_adapter_switch() {
        const host = createTemporaryObject(pageComponent, testCase)
        headphones(host)
        const row = findChild(host.page, "poweredRow")
        verify(row.enabled)
        host.fake.airplane = true
        verify(!row.enabled)
        waitForRendering(host.page)
        mouseClick(findChild(row, "settingSwitch"))
        compare(host.fake.calls, [])
    }

    function test_the_devices_card_shows_a_loading_row_until_the_first_snapshot() {
        const host = createTemporaryObject(pageComponent, testCase)
        host.fake.loaded = false
        const loading = findChild(host.page, "devicesLoading")
        tryCompare(loading, "visible", true)
        verify(!findChild(host.page, "noDevices").visible)
        host.fake.snapshot()
        tryCompare(loading, "visible", false)
    }
}
