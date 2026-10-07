pragma ComponentBehavior: Bound

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
    // Not modal: a modal popup's overlay swallows the wheel as well as the
    // click, and the view behind a menu must keep scrolling. The pointer is
    // still owned while open — by the shield below, which takes every press
    // (closing the menu) and lets wheel events fall through to the content.
    modal: false
    dim: false
    popupType: Popup.Item
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    transformOrigin: Item.TopLeft

    property Item pointerShield: null

    Component {
        id: shieldComponent
        MouseArea {
            anchors.fill: parent
            z: -1
            acceptedButtons: Qt.AllButtons
            onPressed: function(mouse) { mouse.accepted = true; root.close() }
            onWheel: function(wheel) { wheel.accepted = false }
        }
    }

    onOpened: {
        if (!pointerShield && root.Overlay.overlay)
            pointerShield = shieldComponent.createObject(root.Overlay.overlay)
    }
    onClosed: {
        root.besideAnchor = null
        if (pointerShield) {
            pointerShield.destroy()
            pointerShield = null
        }
    }

    // The control the open menu was placed beside, kept while it is open so
    // the menu can be placed again when its height changes.
    property Item besideAnchor: null
    property bool besideAbove: false

    // Opens the menu beside the control that asked for it, never over it:
    // below, or above when preferred or when only above has room. The room is
    // the window's (the overlay), not the menu's parent, which is often a
    // small bar or card; the window edges are kept `margins` clear. The height
    // is the menu's implicit one — `height` is 0 until a first open, which is
    // how menus opened from a button used to land on top of the button — and
    // the menu is placed again whenever that height changes while it is open.
    // `popup(x, y)` stays parent-relative, so the point is mapped back.
    function besidePoint() {
        const ov = root.Overlay.overlay
        const anchor = root.besideAnchor
        const box = anchor.mapToItem(ov, 0, 0)
        const gap = CelestinaTheme.spaceSm
        const edge = root.margins
        const menuHeight = root.implicitHeight
        const below = box.y + anchor.height + gap
        const above = box.y - gap - menuHeight
        const fitsBelow = below + menuHeight <= ov.height - edge
        const fitsAbove = above >= edge
        const goAbove = root.besideAbove ? (fitsAbove || !fitsBelow)
                                         : (!fitsBelow && fitsAbove)
        const x = Math.max(edge, Math.min(box.x, ov.width - edge - root.width))
        return ov.mapToItem(root.parent, x, goAbove ? above : below)
    }

    function popupBeside(anchorItem, preferAbove) {
        root.besideAnchor = anchorItem
        root.besideAbove = !!preferAbove
        const point = root.besidePoint()
        root.popup(point.x, point.y)
    }

    // A menu whose rows arrive from a Repeater or Instantiator learns its
    // real height a frame after it opens: re-place it then, or it opened
    // where a shorter menu would have fit and covered its button.
    onImplicitHeightChanged: {
        if (root.visible && root.besideAnchor) {
            const point = root.besidePoint()
            root.x = point.x
            root.y = point.y
        }
    }

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
