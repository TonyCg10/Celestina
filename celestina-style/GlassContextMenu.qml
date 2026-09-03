import QtQuick
import QtQuick.Controls

// ─── GlassContextMenu ─────────────────────────────────────────────────────────
// A real Menu (focus, Escape, arrows, Enter) whose background is a GlassSurface.
// Stays inside the window scene via Popup.Item. The consumer passes the item to
// blur through `backdropSource`; the menu never captures the compositor window.
// ──────────────────────────────────────────────────────────────────────────────
Menu {
    id: root

    required property Item backdropSource

    width: CelestinaTheme.compMenuWidth
    padding: CelestinaTheme.compMenuPadding
    margins: CelestinaTheme.compMenuMargins
    // Modal without dimming: an open menu must own the pointer, or the click
    // that dismisses it also lands on whatever was underneath — a file gets
    // opened, a drag starts, a row lights up. `dim: false` keeps the look as it
    // was; only the input barrier is new.
    modal: true
    dim: false
    popupType: Popup.Item
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    transformOrigin: Item.TopLeft

    // A nested Menu is represented inside its parent by the parent's delegate.
    // Styling the delegate here keeps cascaded menus in the same glass language
    // instead of letting Qt inject a platform-looking proxy row.
    delegate: GlassMenuItem { }

    // SIMPLE-2 (DRAWING.md): the same panel recipe as every surface — the
    // two-layer shadow and the elevated tint — drawn inline because this
    // file lives in the style module, below the shell's ShellPanel. It
    // carries the dense collector's name so the colour summary lands under
    // it exactly as under a menu.
    background: Item {
        objectName: "celestina-menu-section"
        readonly property real cornerRadius: CelestinaTheme.radiusLg

        CelestinaShadow {
            anchors.fill: parent
            radius: CelestinaTheme.radiusLg
        }

        Rectangle {
            objectName: "celestina-panel-tint"

            anchors.fill: parent
            radius: CelestinaTheme.radiusLg
            antialiasing: true
            color: CelestinaTheme.panelTint
            border.width: CelestinaTheme.borderHairline
            border.color: CelestinaTheme.divider
        }
    }

    // SIMPLE-1 (2026-08-22): one animation for every surface — a plain fade
    // on the shared exit token, in and out. The entry pop and the exit
    // shrink left with the rest of the choreography reset.
    enter: Transition {
        NumberAnimation {
            property: "opacity"
            from: 0
            to: 1
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionExit
            easing.type: CelestinaTheme.easeStandard
        }
    }

    exit: Transition {
        NumberAnimation {
            property: "opacity"
            from: 1
            to: 0
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionExit
            easing.type: CelestinaTheme.easeStandard
        }
    }
}
