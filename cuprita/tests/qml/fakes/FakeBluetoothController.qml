import QtQuick

// Source of truth: celestina-rs/crates/cuprita-core/src/fake.rs. Every row
// and field here mirrors its `scripted()` state (in snapshot order); change
// that file first and this one to match.
// A QML stand-in for BluetoothController over FakeBluetooth's scripted
// state: paired headphones at 80 % and a phone never paired. The pairing
// agent's two signals are raised by the tests themselves.
QtObject {
    id: fake

    property bool powered: true
    property bool discovering: false
    property bool airplane: false
    property var calls: []

    signal agentRequest(string kind, string device, int passkey)
    signal agentClosed()

    property ListModel devices: ListModel {
        ListElement { address: "AA:BB:CC:00:00:01"; name: "Headphones"; kind: "audio"
                      paired: true; connected: false; battery: 80 }
        ListElement { address: "AA:BB:CC:00:00:02"; name: "Phone"; kind: "phone"
                      paired: false; connected: false; battery: -1 }
    }

    function record(call) { fake.calls = fake.calls.concat([call]) }

    function row(address) {
        for (let i = 0; i < fake.devices.count; ++i)
            if (fake.devices.get(i).address === address)
                return i
        return -1
    }

    function setPowered(on) { record("setPowered:" + on); fake.powered = on }
    function setDiscovering(on) { record("setDiscovering:" + on); fake.discovering = on }
    function pair(address) { record("pair:" + address); fake.devices.setProperty(row(address), "paired", true) }
    function connect(address) { record("connect:" + address); fake.devices.setProperty(row(address), "connected", true) }
    function disconnect(address) { record("disconnect:" + address); fake.devices.setProperty(row(address), "connected", false) }
    function answerAgent(kind, value) { record("answerAgent:" + kind + ":" + value) }
    function forget(address) { record("forget:" + address); fake.devices.remove(row(address)) }
}
