import QtQuick
import org.celestina.siderita 1.0

// ─── BottomControls ─────────────────────────────────────────────────────────
// The whole bottom bar: one glass capsule of three icons — what is shown, how
// it is arranged, how big it is. It was four separate pills and some 330px; the
// rest of the width now belongs to the column of transient surfaces on the
// right.
//
// The middle icon *is* the current view mode, so that state is read without
// opening anything; the menu it opens is where sorting lives. Everything from
// outside arrives as a property: this chrome reaches no id of the view that
// instantiates it.
// ──────────────────────────────────────────────────────────────────────────────
GlassPill {
    id: root

    required property var controller
    required property var panel
    required property Item bottomView      // the view the glass samples
    required property bool bottomFloating
    required property Item overlayParent   // where the menu anchors
    required property var viewSortMenu     // the view-and-sort menu
    required property var hostWindow       // the six scales the popup edits

    // Read by the placement above and by the tests, rather than reached into.
    readonly property alias icons: iconRow
    readonly property alias hiddenButton: hiddenIcon
    readonly property alias viewSortButton: viewSortIcon
    readonly property alias sizeButton: sizeIcon
    readonly property alias sizePopup: sizes

    // The glyph's own box inside the capsule, matching the sort group this
    // replaces: the control keeps the suite's hover circle, four pixels in from
    // the capsule's edge.
    readonly property int iconSide: CelestinaTheme.controlHeightSm - 4

    implicitWidth: iconRow.implicitWidth + 2 * CelestinaTheme.spaceXs
    implicitHeight: CelestinaTheme.controlHeightSm
    backdrop: root.bottomView
    floating: root.bottomFloating
    fill: CelestinaTheme.controlFill

    Row {
        id: iconRow
        anchors.centerIn: parent
        spacing: 2

        CelestinaIconButton {
            id: hiddenIcon
            width: root.iconSide
            height: width
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: HiddenToggleDefs.glyph(root.controller.showHidden)
            helpText: HiddenToggleDefs.name(root.controller.showHidden)
            checkable: true
            onClicked: root.controller.toggleHidden()

            // The control's own toggle assigns `checked` on release, which
            // would break a plain binding on the first press and leave the eye
            // stuck on whatever it had become. This restores the binding after
            // each toggle, so the controller stays the one source of truth.
            Binding {
                target: hiddenIcon
                property: "checked"
                value: root.controller.showHidden
                restoreMode: Binding.RestoreBindingOrValue
            }
        }

        CelestinaIconButton {
            id: viewSortIcon
            width: root.iconSide
            height: width
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: root.panel.viewMode === "list"
                      ? "view-list"
                      : root.panel.viewMode === "details"
                        ? "view-details" : "view-grid"
            helpText: qsTr("Vista y orden")
            onClicked: {
                // This control sits at the bottom, so its menu opens upward.
                const menuHeight = root.viewSortMenu.height > 0
                                 ? root.viewSortMenu.height : 300
                const point = viewSortIcon.mapToItem(
                                root.overlayParent, 0, -menuHeight - 6)
                root.viewSortMenu.popup(root.overlayParent, point)
            }
        }

        CelestinaIconButton {
            id: sizeIcon
            width: root.iconSide
            height: width
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: "zoom-in"
            helpText: qsTr("Ajustar tamaños")
            // The popup closes itself on the press that lands outside it — this
            // button included — so by `clicked` it is already closed and a naive
            // toggle reopens it. Decide at press time instead.
            property bool willOpen: false
            onPressedChanged: if (pressed) willOpen = !sizes.opened
            onClicked: willOpen ? sizes.open() : sizes.close()

            SizePopup {
                id: sizes
                y: -height - 10
                // The magnifier moved to the left of the bar, so the popup
                // grows rightwards from the button's own edge. Right-aligning
                // it here, as it was, would push it off the window.
                x: 0
                backdrop: root.panel
                hostWindow: root.hostWindow
            }
        }
    }
}
