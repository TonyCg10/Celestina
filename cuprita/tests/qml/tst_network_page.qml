import QtQuick
import QtTest 1.3
import org.celestina.cuprita 1.0
import "../../qml/pages"
import "fakes"

// The Network page over the scripted networks: four rows with the wired link
// first, and the Wi-Fi switch asks the controller and drops the Wi-Fi rows.
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
}
