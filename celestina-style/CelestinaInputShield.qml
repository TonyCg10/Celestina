import QtQuick
import QtQuick.Templates as T
import QtQuick.Window

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
//
// A third knob for the hover of that same host. Qt delivers hover leaf-first
// and a blocking HoverHandler stops it on the way *up* as much as on the way
// down: a hover-enabled Button whose direct child is this shield never sees
// its own `hovered`. Such a host either sets `blockHover: false` — and accepts
// that hover then also reaches what the button covers, because a Control
// ignores hover events rather than blocking them — or keeps the block and
// paints its hover state from `hovered` here, which is exactly the hover the
// handler took from it.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: shield

    // A surface that is hidden or not painting shields nothing.
    property bool active: true
    property bool swallowClicks: true
    property bool blockHover: true
    // For a host control that owns its own press — a Button with this shield
    // as its child. A handler on a child is visited before its parent item, so
    // this shield's drag handler held a passive grab under the Button's
    // exclusive one, and a zero threshold plus `CanTakeOverFromItems` made it
    // take that grab on the first pixel of movement: the Button lost its press
    // and the click never came — the "sometimes it does not respond" of every
    // floating pill. Such a host already keeps the pointer from what is below,
    // so it yields the drag; a layer that must out-grab its *own* dismissal
    // MouseArea on the way out (the modal) keeps the default.
    property bool yieldsToHost: false
    // Whether the pointer is over this surface, whichever knob is set.
    readonly property alias hovered: hoverGuard.hovered

    anchors.fill: parent
    z: -1
    enabled: shield.active

    HoverHandler {
        id: hoverGuard

        // Stops the row or control underneath from lighting up under a cursor
        // that is really over this surface. `enabled` is explicit: disabling the
        // item does not by itself disarm its handlers.
        enabled: shield.active
        blocking: shield.blockHover
    }

    DragHandler {
        id: dragClaim

        // No properties of its own, and that is load-bearing: a property makes
        // this a type of its own, and a plain DragHandler underneath — which
        // may take over from handlers of a *different* type — then takes the
        // drag back from it. The state lives in `selection` below.
        enabled: shield.active
        target: null
        // Zero threshold, and it matters: the handler underneath starts its own
        // drag after a few pixels, and whoever asks first wins. Claiming the
        // grab on the press is what keeps a sweep that begins on this surface —
        // including one that begins on a control of it and leaves towards the
        // content — from ever reaching that handler.
        //
        // A text field is one of those controls, and it does not keep its grab
        // on the press: Qt's TextInput and TextEdit hold it only past the
        // platform's start-drag distance (about 10 px). Yielding to them would
        // not help — the handler underneath takes the pointer in that gap just
        // the same. So the claim stays here and carries the selection the field
        // was making: a slow sweep moved a pixel at a time and selected nothing.
        dragThreshold: 0
        grabPermissions: shield.yieldsToHost
                         ? PointerHandler.CanTakeOverFromHandlersOfSameType
                           | PointerHandler.CanTakeOverFromHandlersOfDifferentType
                           | PointerHandler.ApprovesTakeOverByAnything
                         : PointerHandler.CanTakeOverFromAnything
                           | PointerHandler.ApprovesTakeOverByAnything

        onActiveChanged: if (active)
                             selection.field = shield.selectingTextAt(
                                 centroid.scenePressPosition)
        onCentroidChanged: {
            const field = selection.field
            if (!field || !dragClaim.active || !shield.holdsLive(field))
                return
            const at = field.mapFromItem(null, centroid.scenePosition.x,
                                         centroid.scenePosition.y)
            field.moveCursorSelection(field.positionAt(at.x, at.y),
                                      field.mouseSelectionMode)
        }
        // The field never saw the release either, and that is where it takes
        // the focus. Only a real release gives it: a grab cancelled or taken
        // over, or a shield or field gone (a dialog accepted mid-drag), gives
        // nothing.
        onGrabChanged: function(transition, point) {
            if (transition !== PointerDevice.UngrabExclusive
                    && transition !== PointerDevice.CancelGrabExclusive)
                return
            const field = selection.field
            selection.field = null
            if (field && transition === PointerDevice.UngrabExclusive
                    && point.state === EventPoint.Released
                    && shield.holdsLive(field) && field.activeFocusOnPress
                    && !field.activeFocus)
                field.forceActiveFocus(Qt.MouseFocusReason)
        }
    }

    QtObject {
        id: selection

        // The text field the drag was taken from, while the claim holds it.
        property Item field: null
    }

    // Whether the drag is still one this shield and that field are part of:
    // both enabled and visible, counting their ancestors.
    function holdsLive(field) {
        return shield.enabled && shield.visible && dragClaim.enabled
                && field.enabled && field.visible
    }

    // The text field a press on this surface started selecting in, if any.
    // The items under the press are walked from the top, the way the press
    // itself was delivered, and the walk ends at the first one that takes a
    // press: a field there is the one; a mouse area, any control (a button,
    // a pane, a menu's padding) or a flickable means the press was not a
    // field's. Those are judged after their own children, so a field inside
    // one is still found first. It also ends at this shield, so a field of the
    // content it covers never qualifies. Anything else just has its pointer
    // held, as before.
    function selectingTextAt(scenePoint) {
        let ended = false
        function visit(item) {
            if (item === shield) {
                ended = true
                return null
            }
            if (!item.visible)
                return null
            const at = item.mapFromItem(null, scenePoint.x, scenePoint.y)
            const under = item.contains(at)
            if (item.clip && !under)
                return null
            const kids = []
            for (let index = 0; index < item.children.length; ++index)
                kids.push({ item: item.children[index], index: index })
            kids.sort((a, b) => b.item.z - a.item.z || b.index - a.index)
            // Children paint over their parent, except those below zero.
            for (const kid of kids) {
                if (kid.item.z < 0)
                    break
                const found = visit(kid.item)
                if (found || ended)
                    return found
            }
            if (under && item.enabled) {
                if (item instanceof TextInput || item instanceof TextEdit) {
                    ended = true
                    return item.selectByMouse ? item : null
                }
                if (item instanceof MouseArea || item instanceof T.Control
                        || item instanceof Flickable) {
                    ended = true
                    return null
                }
            }
            for (const kid of kids) {
                if (kid.item.z >= 0)
                    continue
                const found = visit(kid.item)
                if (found || ended)
                    return found
            }
            return null
        }
        return shield.Window.contentItem ? visit(shield.Window.contentItem) : null
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
