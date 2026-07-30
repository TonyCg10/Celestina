import QtQuick

// A focus-visible outline lives outside its target, so it never competes with
// the target's fill or changes control anatomy. Consumers provide only the
// target and its semantic corner radius; colour and thickness stay canonical.
Rectangle {
    id: ring

    required property Item target
    required property real cornerRadius
    property bool shown: false

    anchors.fill: target
    anchors.margins: -CelestinaTheme.borderFocus
    radius: cornerRadius + CelestinaTheme.borderFocus
    color: CelestinaTheme.clear
    border.width: CelestinaTheme.borderFocus
    border.color: CelestinaTheme.focusRing
    visible: shown && target.visible
    z: 1000
    Accessible.ignored: true
}
