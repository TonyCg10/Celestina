import QtQuick
import QtTest 1.3
import org.celestina.cuprita 1.0
import "../../qml/pages"
import "fakes"

// The Bluetooth page over the scripted devices: two rows, the headphones show
// their battery, and their connect action connects them.
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
}
