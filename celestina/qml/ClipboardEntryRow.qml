// One row of the clipboard history.
//
// Extracted from `ClipboardOverlay` for two reasons, and the second is why it
// is a file rather than an inline delegate: the row carries three input paths
// over the same pixels — select, remove by pointer, remove by button — and
// their stacking is the kind of thing that looks right in a diff and is wrong
// on screen. The row's own `MouseArea` used to be declared *after* the delete
// button, so with equal `z` it sat on top and swallowed every click aimed at
// it. Only a real press finds that; a test that calls the handler directly
// never will. As its own component the row can be pressed in a test window.
//
// The row reports; it never removes or selects anything itself. What those
// words mean belongs to the overlay, and through it to the provider.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: row

    // The entry's fields, as the clipboard provider published them.
    required property var entry
    required property bool current

    signal selected()
    signal removed()

    implicitHeight: 34
    Accessible.role: Accessible.ListItem
    Accessible.name: row.entry.preview
    Accessible.selected: row.current

    Rectangle {
        anchors.fill: parent
        radius: CelestinaTheme.radiusSm
        color: row.current
               ? CelestinaTheme.badgeAccentFill
               : rowMouse.containsMouse
                 ? CelestinaTheme.surfaceHover : CelestinaTheme.clear
    }

    // Clicks only, and it stops where the button starts. Overlapping them does
    // not work however they are stacked: a filling `MouseArea` answered presses
    // aimed at a button drawn above it, by `z` and by declaration order alike,
    // and the visible delete button did nothing at all. The areas are now
    // disjoint, which is the only arrangement that cannot be argued with.
    MouseArea {
        id: rowMouse

        anchors.fill: parent
        // The strip the button occupies is left out. Measured from the
        // button's *implicit* size, not from its position or visibility, so
        // this cannot become a binding cycle through the visibility it feeds.
        anchors.rightMargin: removeButton.implicitWidth + CelestinaTheme.spaceXs
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: function(mouse) {
            if (mouse.button === Qt.RightButton)
                row.removed();
            else
                row.selected();
        }
    }

    Text {
        x: CelestinaTheme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width - CelestinaTheme.spaceSm * 2 - removeButton.width
        text: row.entry.preview
        color: row.current ? CelestinaTheme.accent : CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowSecondary
        elide: Text.ElideRight
    }

    // Deleting used to be reachable only by the Delete key or a right-click,
    // neither of which a person can see. This is the same action with a shape
    // on screen; the keyboard and context-menu paths are unchanged.
    CelestinaIconButton {
        id: removeButton

        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceXs
        anchors.verticalCenter: parent.verticalCenter
        iconName: "x"
        // Shown on the row the keyboard is on and on the row under the
        // pointer. `hovered` is part of that test because the pointer moving
        // onto the button takes the hover away from the area underneath, and a
        // button that vanished as you reached for it would be worse than none.
        // The row's area stops short of this button, so its `containsMouse`
        // is false while the pointer is here — the button's own `hovered` is
        // what keeps it on screen as you reach for it.
        visible: row.current || rowMouse.containsMouse || hovered
        activeFocusOnTab: visible
        Accessible.name: qsTr("Quitar «%1» del historial").arg(row.entry.preview)
        helpText: qsTr("Quitar esta entrada del historial")
        onClicked: row.removed()
    }
}
