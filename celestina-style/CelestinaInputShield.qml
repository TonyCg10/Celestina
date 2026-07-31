import QtQuick

// ─── CelestinaInputShield ─────────────────────────────────────────────────────
// The input floor of a surface that floats over live content. A box that only
// paints is not a surface: whatever it covers keeps its hover, its clicks — all
// three buttons — and, the one that actually bites, its pointer *handlers*. A
// `DragHandler` under a dialog or a pill takes a passive grab on press and keeps
// reacting, so a sweep over the box drags an item the user cannot even see.
//
// It sits at `z: -1`, below its surface's own children: the controls inside are
// delivered first and stay fully interactive, and only what they did not claim
// is absorbed here. The wheel is deliberately left alone — content is meant to
// keep scrolling under floating chrome.
//
// Two knobs, because two shapes of consumer exist: a surface that owns nothing
// (a pill, a banner) wants the whole thing, while a control that already
// handles its own press (a Button) or a layer with its own dismissal MouseArea
// wants `swallowClicks: false` — their press handling lives on the item *below*
// these children, so a swallowing MouseArea here would eat the click first.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: shield

    // A surface that is hidden or not painting shields nothing.
    property bool active: true
    property bool swallowClicks: true

    anchors.fill: parent
    z: -1
    enabled: shield.active

    HoverHandler {
        // Stops the row or control underneath from lighting up under a cursor
        // that is really over this surface. `enabled` is explicit: disabling the
        // item does not by itself disarm its handlers.
        enabled: shield.active
        blocking: true
    }

    DragHandler {
        enabled: shield.active
        target: null
        // Zero threshold, and it matters: the handler underneath starts its own
        // drag after a few pixels, and whoever asks first wins. Claiming the
        // grab on the press is what keeps a sweep that begins on this surface —
        // including one that begins on a control of it and leaves towards the
        // content — from ever reaching that handler. Controls keep what is
        // theirs: a text field holds its own grab while it selects.
        dragThreshold: 0
        grabPermissions: PointerHandler.CanTakeOverFromAnything
                         | PointerHandler.ApprovesTakeOverByAnything
    }

    MouseArea {
        anchors.fill: parent
        // Disabling the root item is not enough: hover is still delivered to a
        // disabled item, so an inactive shield would keep stealing it from what
        // is underneath. Each condition rides the property that governs it.
        enabled: shield.active && shield.swallowClicks
        // All three buttons: a right click landing on an item behind would open
        // that item's menu, and a middle click would open it elsewhere. Those
        // are exactly the surprises this layer exists to prevent.
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        hoverEnabled: enabled
        preventStealing: true
    }
}
