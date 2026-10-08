import QtQuick
import QtTest 1.3
import org.celestina.cuprita 1.0
import "../../qml/pages"
import "fakes"

// The Network page over the scripted networks: four rows with the wired link
// first, the Wi-Fi switch asks the controller and drops the Wi-Fi rows, an
// open network joins at once and a protected one without a saved profile
// asks for its passphrase first.
TestCase {
    id: testCase
    name: "NetworkPage"
    width: 640
    height: 480
    visible: true
    when: windowShown

    Component {
        id: pageComponent

        Item {
            id: host
            property alias page: networkPage
            property alias fake: controller
            width: 640
            height: 480

            FakeNetworkController { id: controller }

            NetworkPage {
                id: networkPage
                anchors.fill: parent
                controller: controller
                networks: controller.networks
                backdropSource: host
            }
        }
    }

    function test_four_rows_wired_first() {
        const host = createTemporaryObject(pageComponent, testCase)
        const list = findChild(host.page, "networkList")
        verify(list)
        tryCompare(list, "count", 4)
        list.currentIndex = 0
        const first = list.itemAtIndex(0)
        verify(first)
        compare(first.model.kind, "ethernet")
        compare(findChild(first, "networkState").text, "Conectado")
    }

    function test_wifi_switch_asks_and_hides_wifi_rows() {
        const host = createTemporaryObject(pageComponent, testCase)
        const list = findChild(host.page, "networkList")
        tryCompare(list, "count", 4)
        const toggle = findChild(findChild(host.page, "wifiRow"), "settingSwitch")
        verify(toggle.checked)
        waitForRendering(host.page)
        mouseClick(toggle)
        compare(host.fake.calls, ["setWifiEnabled:false"])
        tryCompare(list, "count", 2)
        verify(!toggle.checked)
        for (let i = 0; i < host.fake.networks.count; ++i)
            verify(host.fake.networks.get(i).kind !== "wifi")
    }

    function rowWithId(list, id) {
        for (let i = 0; i < list.count; ++i) {
            list.currentIndex = i
            const row = list.itemAtIndex(i)
            if (row && row.model.id === id)
                return row
        }
        return null
    }

    function test_open_network_connects_without_a_dialog() {
        const host = createTemporaryObject(pageComponent, testCase)
        const list = findChild(host.page, "networkList")
        tryCompare(list, "count", 4)
        const row = rowWithId(list, "open-cafe")
        verify(row)
        waitForRendering(host.page)
        mouseClick(findChild(row, "joinButton"))
        compare(host.fake.calls, ["connect:open-cafe:"])
        verify(!findChild(host.page, "wifiPasswordDialog").shown)
    }

    function test_unknown_protected_network_asks_for_the_password() {
        const host = createTemporaryObject(pageComponent, testCase)
        const list = findChild(host.page, "networkList")
        tryCompare(list, "count", 4)
        // Not in the scripted state: a protected network nobody saved.
        host.fake.networks.append({ id: "neighbour", name: "Neighbour", kind: "wifi",
                                    state: "disconnected", signal: 50, bars: 2,
                                    security: "psk", known: false })
        tryCompare(list, "count", 5)
        const row = rowWithId(list, "neighbour")
        verify(row)
        waitForRendering(host.page)
        mouseClick(findChild(row, "joinButton"))
        const dialog = findChild(host.page, "wifiPasswordDialog")
        tryCompare(dialog, "shown", true)
        compare(host.fake.calls, [])
        const field = findChild(dialog, "passwordField")
        compare(field.echoMode, TextInput.Password)
        tryCompare(field, "activeFocus", true)
        keyClick(Qt.Key_S)
        keyClick(Qt.Key_E)
        keyClick(Qt.Key_C)
        keyClick(Qt.Key_R)
        keyClick(Qt.Key_E)
        keyClick(Qt.Key_T)
        compare(field.text, "secret")
        mouseClick(findChild(dialog, "revealButton"))
        compare(field.echoMode, TextInput.Normal)
        const confirm = findChild(dialog, "confirmButton")
        waitForRendering(host.page)
        mouseClick(confirm)
        compare(host.fake.calls, ["connect:neighbour:secret"])
        verify(!confirm.enabled)
        tryCompare(dialog, "shown", false)
        compare(field.text, "")
    }

    function test_a_connecting_row_ignores_another_join() {
        const host = createTemporaryObject(pageComponent, testCase)
        const list = findChild(host.page, "networkList")
        tryCompare(list, "count", 4)
        const row = rowWithId(list, "open-cafe")
        verify(row)
        host.fake.networks.setProperty(row.index, "state", "connecting")
        waitForRendering(host.page)
        mouseClick(findChild(row, "joinButton"))
        compare(host.fake.calls, [])
    }

    function test_a_join_in_flight_ignores_a_second_press() {
        const host = createTemporaryObject(pageComponent, testCase)
        const list = findChild(host.page, "networkList")
        tryCompare(list, "count", 4)
        const row = rowWithId(list, "open-cafe")
        verify(row)
        host.fake.busy = true
        waitForRendering(host.page)
        const join = findChild(row, "joinButton")
        mouseClick(join)
        mouseClick(join)
        compare(host.fake.calls, ["connect:open-cafe:"])
        host.fake.busy = false
        mouseClick(join)
        compare(host.fake.calls, ["connect:open-cafe:", "connect:open-cafe:"])
    }

    function test_a_notice_after_a_join_re_enables_the_row() {
        const host = createTemporaryObject(pageComponent, testCase)
        const list = findChild(host.page, "networkList")
        tryCompare(list, "count", 4)
        const row = rowWithId(list, "open-cafe")
        verify(row)
        // Busy never drops: the command could not be queued.
        host.fake.busy = true
        waitForRendering(host.page)
        const join = findChild(row, "joinButton")
        mouseClick(join)
        mouseClick(join)
        compare(host.fake.calls, ["connect:open-cafe:"])
        host.fake.notice("error", "unavailable")
        mouseClick(join)
        compare(host.fake.calls, ["connect:open-cafe:", "connect:open-cafe:"])
    }
}
