import QtQuick

// A QML stand-in for AppearanceController over the fake store's starting
// state (backend.rs `FakeAppearance`): the default appearance, read at once.
// Each command records itself and answers at once, as the controller does
// once its worker reads the file back.
QtObject {
    id: fake

    property bool reducedMotion: false
    property string textScale: "normal"
    property bool forcedByEnvironment: false
    // A test may set it false to see the loading row.
    property bool loaded: true
    property var calls: []
    // A test sets it to make every save fail: the value stays and one
    // error notice is raised, as the controller does when `save` fails.
    property bool failing: false

    signal notice(string kind, string text)

    function record(call) { fake.calls = fake.calls.concat([call]) }

    function refuse() { fake.notice("error", "No se pudo guardar la apariencia") }
    function setReducedMotion(on) {
        record("setReducedMotion:" + on)
        if (fake.failing) refuse(); else fake.reducedMotion = on
    }
    function setTextScale(name) {
        record("setTextScale:" + name)
        if (fake.failing) refuse(); else fake.textScale = name
    }
}
