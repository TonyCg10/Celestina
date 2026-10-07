import QtQuick
import QtQuick.Controls

// A section header inside a GlassContextMenu. It is a real MenuItem, so it
// keeps its place among items a Repeater or Instantiator adds later (a plain
// label child was moved to the end of the menu), and it is disabled so the
// arrow keys skip it. Hidden, it takes no room.
MenuItem {
    id: control

    enabled: false
    focusPolicy: Qt.NoFocus
    Accessible.role: Accessible.StaticText
    implicitWidth: CelestinaTheme.compMenuWidth - CelestinaTheme.compMenuPadding * 2
    implicitHeight: visible ? label.implicitHeight + topPadding + bottomPadding : 0
    leftPadding: CelestinaTheme.spaceMd
    rightPadding: CelestinaTheme.spaceMd
    topPadding: CelestinaTheme.spaceSm
    bottomPadding: CelestinaTheme.spaceXs
    indicator: null
    arrow: null
    background: null
    contentItem: CelestinaSectionLabel {
        id: label
        text: control.text
        elide: Text.ElideRight
    }
}
