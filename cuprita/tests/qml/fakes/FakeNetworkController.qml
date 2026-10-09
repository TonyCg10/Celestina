import QtQuick

// Source of truth: celestina-rs/crates/cuprita-core/src/fake.rs. Every row
// and field here mirrors its `scripted()` state (in snapshot order); change
// that file first and this one to match.
// A QML stand-in for NetworkController over the same scripted state as
// cuprita-core's FakeNetwork: a wired link in use at 192.168.1.23, a saved PSK Wi-Fi, an open
// Wi-Fi and a VPN. It records every call and mutates its rows like the fake.
QtObject {
    id: fake

    property bool wifiEnabled: true
    property bool airplane: false
    // Never busy: the stand-in answers every command at once.
    property bool busy: false
    // The first snapshot has arrived. The stand-in starts with it, like the
    // fakes; a test may set it false and call `snapshot()` to deliver it.
    // Like the controller, the rows land before `loaded` turns true.
    property bool loaded: true
    function snapshot() { reload(); loaded = true }
    property var calls: []

    signal notice(string kind, string text)

    property ListModel networks: ListModel { }

    readonly property var scripted: [
        { id: "wired", name: "Ethernet", kind: "ethernet", state: "connected",
          signal: -1, bars: 0, security: "open", known: true, address: "192.168.1.23" },
        { id: "home-wifi", name: "Home", kind: "wifi", state: "disconnected",
          signal: 72, bars: 3, security: "psk", known: true, address: "" },
        { id: "office-vpn", name: "Office", kind: "vpn", state: "disconnected",
          signal: -1, bars: 0, security: "open", known: true, address: "" },
        { id: "open-cafe", name: "Corner Cafe", kind: "wifi", state: "disconnected",
          signal: 20, bars: 1, security: "open", known: false, address: "" }
    ]

    function record(call) { fake.calls = fake.calls.concat([call]) }

    function reload() {
        fake.networks.clear()
        for (let i = 0; i < fake.scripted.length; ++i) {
            const n = fake.scripted[i]
            if (n.kind !== "wifi" || (fake.wifiEnabled && !fake.airplane))
                fake.networks.append(n)
        }
    }

    function setWifiEnabled(on) { record("setWifiEnabled:" + on); fake.wifiEnabled = on; reload() }
    function setAirplane(on) { record("setAirplane:" + on); fake.airplane = on; reload() }
    // The password is recorded so a test can see what the page passed on.
    function connect(id, password) { record("connect:" + id + ":" + password) }
    function disconnect(id) { record("disconnect:" + id) }
    function forget(id) { record("forget:" + id) }
    function setVpnActive(id, on) { record("setVpnActive:" + id + ":" + on) }

    Component.onCompleted: reload()
}
