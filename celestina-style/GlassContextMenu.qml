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

    // The glass this component is named for. The 2026-08-29 drawing milestone
    // flattened this into the shell's shadow-and-tint card, which silently
    // stripped the blur from every application's context menu — DRAWING.md
    // governs the shell's own cards (MenuSection, ShellPanel keep theirs),
    // not this shared control. Restored by the author's decision, 2026-09-03.
    background: GlassSurface {
        backdropSource: root.backdropSource
        captureEnabled: root.visible
        cornerRadius: CelestinaTheme.radiusLg
        // A menu is a floating layer (L2) — the drop shadow reads as hovering
        // over the content instead of pasted onto it.
        elevation: 2
        // The content behind a menu keeps moving while it is open — the wheel
        // still scrolls the view, thumbnails arrive, rows light up under the
        // cursor. A one-shot capture froze all of that, so the menu wore a
        // blurred screenshot of the instant it opened instead of real glass.
        liveCapture: true
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
