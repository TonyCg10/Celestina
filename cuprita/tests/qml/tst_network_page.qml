import QtQuick
import QtTest 1.3
import org.celestina.cuprita 1.0
import "../../qml/pages"
import "fakes"

// The Network page over the scripted networks: the wired link in use with its
// address in the connection card, the two Wi-Fi networks in the networks card,
// the VPN in its own card; the Wi-Fi switch asks the controller and drops the Wi-Fi
// rows, an open network joins at once and a protected one without a saved
// profile asks for its passphrase first.
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

    function test_the_connection_card_shows_the_wired_link_and_its_address() {
        const host = createTemporaryObject(pageComponent, testCase)
        const card = findChild(host.page, "connectionCard")
        verify(card)
        tryVerify(function() { return findChild(card, "connectionRow") })
        const row = findChild(card, "connectionRow")
        compare(findChild(row, "connectionName").text, "Ethernet")
        compare(findChild(row, "connectionDetail").text, "Ethernet · 192.168.1.23")
        verify(!findChild(card, "noConnection").visible)
        waitForRendering(host.page)
        mouseClick(findChild(row, "disconnectButton"))
        compare(host.fake.calls, ["disconnect:wired"])
    }

    function test_the_networks_card_leaves_out_the_connection_and_the_vpn() {
        const host = createTemporaryObject(pageComponent, testCase)
        const list = findChild(host.page, "networkList")
        verify(list)
        tryCompare(list, "count", 2)
        const ids = []
        for (let i = 0; i < list.count; ++i) {
            list.currentIndex = i
            ids.push(list.itemAtIndex(i).model.id)
        }
        compare(ids, ["home-wifi", "open-cafe"])
        // The model keeps every row; only the page filters.
        compare(host.fake.networks.count, 4)
        compare(findChild(list.itemAtIndex(0), "networkState").text, "Protegida")
        compare(findChild(list.itemAtIndex(0), "signalBars").level, 3)
    }

    function test_without_a_connection_the_card_says_so() {
        const host = createTemporaryObject(pageComponent, testCase)
        const list = findChild(host.page, "networkList")
        tryCompare(list, "count", 2)
        host.fake.networks.setProperty(0, "state", "disconnected")
        host.fake.networks.setProperty(0, "address", "")
        const card = findChild(host.page, "connectionCard")
        tryCompare(findChild(card, "noConnection"), "visible", true)
        verify(!findChild(card, "connectionRow"))
        // The wire leaves the card for the list.
        tryCompare(list, "count", 3)
    }

    function test_the_vpn_card_switches_the_vpn() {
        const host = createTemporaryObject(pageComponent, testCase)
        const card = findChild(host.page, "vpnCard")
        tryCompare(card, "visible", true)
        tryVerify(function() { return findChild(card, "vpnRow") })
        const row = findChild(card, "vpnRow")
        compare(row.label, "Office")
        const toggle = findChild(row, "settingSwitch")
        verify(!toggle.checked)
        compare(toggle.Accessible.name, "Activar Office")
        waitForRendering(host.page)
        mouseClick(toggle)
        compare(host.fake.calls, ["setVpnActive:office-vpn:true"])
    }

    function test_wifi_switch_asks_and_hides_wifi_rows() {
        const host = createTemporaryObject(pageComponent, testCase)
        const list = findChild(host.page, "networkList")
        tryCompare(list, "count", 2)
        const toggle = findChild(findChild(host.page, "wifiRow"), "settingSwitch")
        verify(toggle.checked)
        // The scene is idle by now (the count wait let it render); a frame
        // wait would only time out. The click needs the switch laid out.
        tryVerify(function() { return toggle.visible && toggle.width > 0 && toggle.height > 0 })
        mouseClick(toggle)
        compare(host.fake.calls, ["setWifiEnabled:false"])
        // Only the wire (in the connection card) and the VPN are left.
        tryCompare(list, "count", 0)
        tryCompare(findChild(host.page, "noNetworks"), "visible", true)
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
        tryCompare(list, "count", 2)
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
        tryCompare(list, "count", 2)
        // Not in the scripted state: a protected network nobody saved.
        host.fake.networks.append({ id: "neighbour", name: "Neighbour", kind: "wifi",
                                    state: "disconnected", signal: 50, bars: 2,
                                    security: "psk", known: false, address: "" })
        tryCompare(list, "count", 3)
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
        // Six characters: too short for a WPA passphrase.
        const confirm = findChild(dialog, "confirmButton")
        verify(!confirm.enabled)
        verify(findChild(dialog, "lengthHint").visible)
        keyClick(Qt.Key_Return)
        compare(host.fake.calls, [])
        keyClick(Qt.Key_1)
        keyClick(Qt.Key_2)
        compare(field.text, "secret12")
        verify(confirm.enabled)
        verify(!findChild(dialog, "lengthHint").visible)
        mouseClick(findChild(dialog, "revealButton"))
        compare(field.echoMode, TextInput.Normal)
        waitForRendering(host.page)
        mouseClick(confirm)
        compare(host.fake.calls, ["connect:neighbour:secret12"])
        verify(!confirm.enabled)
        tryCompare(dialog, "shown", false)
        compare(field.text, "")
    }

    function test_the_passphrase_length_gates_connect() {
        const host = createTemporaryObject(pageComponent, testCase)
        const dialog = findChild(host.page, "wifiPasswordDialog")
        dialog.ask("neighbour", "Neighbour")
        tryCompare(dialog, "shown", true)
        const field = findChild(dialog, "passwordField")
        const confirm = findChild(dialog, "confirmButton")
        const hint = findChild(dialog, "lengthHint")
        verify(field.inputMethodHints & Qt.ImhSensitiveData)
        verify(field.inputMethodHints & Qt.ImhNoPredictiveText)
        // Empty: disabled, but no hint yet.
        verify(!confirm.enabled)
        verify(!hint.visible)
        field.text = "1234567"
        verify(!confirm.enabled)
        verify(hint.visible)
        field.text = "12345678"
        verify(confirm.enabled)
        field.text = "a".repeat(63)
        verify(confirm.enabled)
        field.text = "g".repeat(64)
        verify(!confirm.enabled)
        verify(hint.visible)
        // Sixty-four hexadecimal digits are the raw key.
        field.text = "0123456789abcdef".repeat(4)
        verify(confirm.enabled)
        field.text = "g".repeat(70)
        dialog.confirm()
        compare(host.fake.calls, [])
        dialog.close()
    }

    function test_a_connecting_row_ignores_another_join() {
        const host = createTemporaryObject(pageComponent, testCase)
        const list = findChild(host.page, "networkList")
        tryCompare(list, "count", 2)
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
        tryCompare(list, "count", 2)
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
        tryCompare(list, "count", 2)
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

    function test_the_networks_card_shows_a_loading_row_until_the_first_snapshot() {
        const host = createTemporaryObject(pageComponent, testCase)
        host.fake.loaded = false
        const loading = findChild(host.page, "networksLoading")
        tryCompare(loading, "visible", true)
        verify(!findChild(host.page, "noNetworks").visible)
        verify(!findChild(host.page, "noConnection").visible)
        host.fake.snapshot()
        tryCompare(loading, "visible", false)
        // A command in flight: a spinner beside the card's label only.
        host.fake.busy = true
        tryCompare(findChild(findChild(host.page, "connectionCard"), "workingSpinner"), "visible", true)
        verify(!loading.visible)
    }

    function test_the_empty_line_never_shows_between_loaded_and_the_rows() {
        const host = createTemporaryObject(pageComponent, testCase)
        host.fake.loaded = false
        host.fake.networks.clear()
        const empty = findChild(host.page, "noNetworks")
        const list = findChild(host.page, "networkList")
        tryCompare(findChild(host.page, "networksLoading"), "visible", true)
        let flashed = false
        empty.visibleChanged.connect(function() { if (empty.visible) flashed = true })
        host.fake.snapshot()
        tryCompare(list, "count", 2)
        // Let any late pass run: a flash would show in these turns.
        wait(CelestinaTheme.motionFast)
        verify(!flashed)
        verify(!empty.visible)
    }
}
