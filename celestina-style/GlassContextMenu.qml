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
    modal: false
    dim: false
    popupType: Popup.Item
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    transformOrigin: Item.TopLeft

    // A nested Menu is represented inside its parent by the parent's delegate.
    // Styling the delegate here keeps cascaded menus in the same glass language
    // instead of letting Qt inject a platform-looking proxy row.
    delegate: GlassMenuItem { }

    background: GlassSurface {
        id: glassBackground
        backdropSource: root.backdropSource
        captureEnabled: root.visible
        // A menu is a floating layer (L2) — give it the drop shadow so it reads
        // as hovering over the content instead of pasted onto it.
        elevation: 2
        // The content behind a menu keeps moving while it is open — the wheel
        // still scrolls the view, thumbnails arrive, rows light up under the
        // cursor. A one-shot capture froze all of that, so the menu wore a
        // blurred screenshot of the instant it opened instead of real glass.
        liveCapture: true
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
    // …and again whenever the overlay moves the menu to keep it on screen. The
    // surface itself re-samples on a size change; only its position is news
    // that has to come from here.
    onXChanged: glassBackground.refreshBackdrop()
    onYChanged: glassBackground.refreshBackdrop()
}
