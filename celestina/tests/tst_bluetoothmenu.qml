import QtQuick
import QtTest
import "../qml" as Desktop

// Request lifecycle — sending, waiting, confirming, failing, generation loss —
// is not tested here any more. It cannot be: activating a row destroys this
// window, so a case that keeps the object alive and hand-delivers a result is
// testing something the product never does. It lives on the durable ledger and
// on the real menu lifecycle instead:
// `requestledger_test.cpp` and `indicatormenu_test.cpp`.

// What the Bluetooth menu shows, and what it refuses to show.
//
// Constructed offscreen: this proves the reading and request rules, never the
// appearance. Nothing here starts discovery, pairs, forgets or trusts anything,
// because the menu has no way to ask for those at all.
TestCase {
    id: testCase

    name: "BluetoothMenu"

    QtObject {
        id: fakeSource

        property bool available: true
        property var providers: ({})
        property var requests: null
        property int revision: 0
        property var sent: []
        property int nextId: 1

        signal changed()
        signal commandResult(int requestId, string state, string reason)

        function publish(next) {
            fakeSource.providers = next;
            fakeSource.revision = fakeSource.revision + 1;
            fakeSource.changed();
        }

        function sendCommand(provider, verb, options) {
            fakeSource.sent.push({
                "provider": provider, "verb": verb, "options": options
            });
            return fakeSource.nextId++;
        }
    }

    Desktop.BluetoothMenu {
        id: menu

        outputName: "test-output"
        providerSource: fakeSource
        reducedMotion: true
    }

    readonly property var known: [
        {
            "id": "5C:DC:49:0D:D1:62", "name": "S25 Ultra",
            "connected": false, "paired": true
        },
        {
            "id": "AA:BB:CC:DD:EE:01", "name": "WH-1000XM4",
            "connected": true, "paired": true
        }
    ]

    function publishPowered(devices, listState) {
        fakeSource.publish({
            "bluetooth": {
                "adapter": "on",
                "count": 1,
                "first": "WH-1000XM4",
                "devicesState": listState === undefined ? "fresh" : listState,
                "devices": devices === undefined ? testCase.known : devices
            }
        });
    }

    function entriesOfKind(kind) {
        const found = [];
        for (let index = 0; index < menu.entries.length; ++index) {
            if (menu.entries[index].kind === kind)
                found.push(menu.entries[index]);
        }
        return found;
    }

    function init() {
        fakeSource.sent = [];
        fakeSource.nextId = 1;
        fakeSource.available = true;
        testCase.publishPowered();
    }

    function test_the_inventory_has_deliberate_vertical_rhythm() {
        compare(menu.itemSpacing, 8);
        compare(menu.headerBodyGap, 12);
        compare(menu.rowVerticalInset, 4);

        const header = menu.menu.itemAt(0);
        const firstBodyRow = menu.menu.itemAt(1);
        verify(header);
        verify(firstBodyRow);
        compare(header.headerTrailingGap, 12);
        compare(header.visualHeight, menu.headerRowHeight);
        compare(header.implicitHeight - header.visualHeight,
                menu.headerBodyGap);
        compare(firstBodyRow.verticalInset, 4);
        compare(firstBodyRow.implicitHeight - firstBodyRow.visualHeight,
                menu.itemSpacing);
    }

    // The policy UX-1-A closed: a powered adapter with nothing on it is a state
    // a person needs to see, and the menu says so rather than going blank.
    function test_a_powered_adapter_with_no_devices_still_shows_itself() {
        testCase.publishPowered([]);

        compare(menu.adapter, "on");
        compare(menu.powered, true);
        compare(menu.switchable, true);
        compare(menu.adapterLine, qsTr("Bluetooth encendido"));
        compare(menu.listLine, qsTr("No hay dispositivos conocidos"));
        // The switch is there, and so is the refresh.
        compare(testCase.entriesOfKind("adapter").length, 1);
        compare(testCase.entriesOfKind("refresh").length, 1);
        compare(testCase.entriesOfKind("device").length, 0);
    }

    function test_an_adapter_that_is_off_is_not_one_that_is_missing() {
        fakeSource.publish({"bluetooth": {"adapter": "off", "count": 0}});
        compare(menu.adapterLine, qsTr("Bluetooth apagado"));
        compare(menu.switchable, true);
        compare(menu.powered, false);
        // A radio that is off has nothing on it, and says nothing about
        // devices rather than claiming there are none.
        compare(menu.listLine, "");
        compare(testCase.entriesOfKind("device").length, 0);

        fakeSource.publish({"bluetooth": {"adapter": "absent", "count": 0}});
        compare(menu.adapterLine, qsTr("Este equipo no tiene Bluetooth"));
        // Nothing to switch on a machine with no controller.
        compare(menu.switchable, false);

        fakeSource.publish({});
        compare(menu.adapter, "");
        compare(menu.adapterLine, qsTr("Sin información de Bluetooth"));
        compare(menu.switchable, false);
    }

    function test_every_list_state_has_its_own_spanish_sentence() {
        const said = {};
        const states = ["fresh", "held", "unavailable", "pending"];
        for (let index = 0; index < states.length; ++index) {
            testCase.publishPowered(testCase.known, states[index]);
            verify(menu.listLine.length > 0);
            verify(said[menu.listLine] === undefined, menu.listLine);
            said[menu.listLine] = true;
        }

        testCase.publishPowered(testCase.known, "unavailable");
        compare(menu.listLine, qsTr("No se puede consultar: falta bluetoothctl"));
        testCase.publishPowered(testCase.known, "held");
        compare(menu.listLine, qsTr("Dispositivos conocidos (lectura anterior)"));
    }

    function test_a_device_row_carries_the_state_bluez_confirmed() {
        // The entry list carries identity only; the live device behind each
        // identity is looked up, so a reading tick moves state without
        // recreating the row that shows it.
        const devices = testCase.entriesOfKind("device");
        compare(devices.length, 2);
        compare(menu.deviceById(devices[0].id).name, "S25 Ultra");
        compare(menu.deviceById(devices[0].id).connected, false);
        compare(menu.deviceById(devices[1].id).connected, true);
    }

    function test_a_reading_tick_moves_state_without_rebuilding_rows() {
        const before = [];
        for (let index = 0; index < menu.menu.count; ++index)
            before.push(menu.menu.itemAt(index));
        verify(before.length > 0);
        const signature = menu.entrySignature;

        // Republish the same inventory with one device's state flipped, the
        // way a provider tick does. The rows must be the same objects.
        const flipped = [
            {"id": testCase.known[0].id, "name": testCase.known[0].name,
             "connected": true},
            {"id": testCase.known[1].id, "name": testCase.known[1].name,
             "connected": testCase.known[1].connected}
        ];
        testCase.publishPowered(flipped, "fresh");

        compare(menu.entrySignature, signature);
        compare(menu.menu.count, before.length);
        for (let index = 0; index < before.length; ++index) {
            verify(menu.menu.itemAt(index) === before[index],
                   "row " + index + " was recreated by a reading tick");
        }
        compare(menu.deviceById(testCase.known[0].id).connected, true);
    }










}
