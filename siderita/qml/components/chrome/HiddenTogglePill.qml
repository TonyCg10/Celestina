import QtQuick
import org.celestina.siderita 1.0

// The one visual contract for revealing dotfiles in every Siderita browser.
FloatingButton {
    id: control

    required property bool toggleChecked
    property real textScale: 1.0

    signal toggleRequested

    // Icon-only: the eye says it, and HiddenToggleDefs says what a screen
    // reader hears. The capsule in the folder view draws the same toggle as a
    // ghost icon, so the glyph and the name have one owner rather than two.
    iconName: HiddenToggleDefs.glyph(control.toggleChecked)
    helpText: HiddenToggleDefs.name(control.toggleChecked)
    active: toggleChecked
    font.pixelSize: Math.round(CelestinaTheme.fontMini * textScale)
    Accessible.name: HiddenToggleDefs.name(control.toggleChecked)
    Accessible.checkable: true
    Accessible.checked: toggleChecked
    onClicked: toggleRequested()
}
