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

    width: CelestinaTheme.menuWidth
    padding: CelestinaTheme.menuPadding
    margins: CelestinaTheme.menuMargins
    modal: false
    dim: false
    popupType: Popup.Item
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    transformOrigin: Item.TopLeft

    background: GlassSurface {
        id: glassBackground
        backdropSource: root.backdropSource
        captureEnabled: root.visible
    }

    enter: Transition {
        ParallelAnimation {
            NumberAnimation {
                property: "opacity"
                from: 0
                to: 1
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
            NumberAnimation {
                property: "scale"
                from: 0.96
                to: 1
                duration: CelestinaTheme.motionNormal
                easing.type: CelestinaTheme.easeEmphasized
                easing.overshoot: CelestinaTheme.overshoot
            }
        }
    }

    exit: Transition {
        ParallelAnimation {
            NumberAnimation {
                property: "opacity"
                from: 1
                to: 0
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeExit
            }
            NumberAnimation {
                property: "scale"
                from: 1
                to: 0.98
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeExit
            }
        }
    }

    onAboutToShow: Qt.callLater(function() {
        glassBackground.refreshBackdrop()
    })
    // Re-sample once the menu has its final position (aboutToShow fires before
    // x/y are set), so the blur matches what is actually behind it — not a
    // stale region captured at the origin.
    onOpened: Qt.callLater(function() {
        glassBackground.refreshBackdrop()
    })
}
