// The interaction contract for a panel control that opens a transient surface.
//
// CelestinaButton owns the common hover, pressed and keyboard-focus anatomy.
// This specialization snapshots both the complete control that was invoked
// and the exact icon inside it. The former places the contextual body; the
// latter positions the narrow waist of the background-only membrane.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

BackdropButton {
    id: root

    required property Item attachmentAnchor
    readonly property bool isPanelAttachmentSource: true
    // Set only by the tokened attachment lease that owns the currently mapped
    // contextual surface. The inherited background then keeps the ordinary
    // hover circle visible until that exact surface retires.
    property bool menuOpen: false
    // Opt-in, never the default: a pointer resting on this control opens what
    // a click would. Only the three reading openers take it (2026-08-14), and
    // only after a dwell — a pointer crossing the bar on its way somewhere
    // else passes over every control in the row, and opening on contact would
    // make the bar unusable to walk across.
    property bool opensOnHover: false
    // Whether what is currently up was opened by the pointer resting here
    // rather than by a click. A click on an opener whose own menu is already
    // showing puts it away — the toggle every opener has — but the pointer
    // arriving is not a request to put anything away, so the click that
    // merely confirms what the dwell already did is spent instead of
    // answered. It hands the toggle back at once: the next click closes.
    property bool openedByHover: false

    signal menuRequested(rect openerRect, rect attachmentAnchorRect)

    function globalRect(item) {
        const topLeft = item.mapToGlobal(0, 0);
        const bottomRight = item.mapToGlobal(item.width, item.height);
        return Qt.rect(Math.min(topLeft.x, bottomRight.x),
                       Math.min(topLeft.y, bottomRight.y),
                       Math.abs(bottomRight.x - topLeft.x),
                       Math.abs(bottomRight.y - topLeft.y));
    }

    function attachmentAnchorGlobalRectNow() {
        return root.globalRect(root.attachmentAnchor);
    }

    function requestMenu() {
        root.menuRequested(root.globalRect(root),
                           root.attachmentAnchorGlobalRectNow());
    }

    height: CelestinaTheme.controlHeightXs
    density: CelestinaButton.Compact
    role: CelestinaButton.Ghost

    // One circle behind every opener, the author's stated hierarchy: a panel
    // control that opens something is an icon in a circle of this exact
    // height, and a control that carries two icons reads as one capsule twice
    // as long — the same shape, stretched, never two circles or a rounded
    // square. The base radius is a token for cards; the capsule radius makes a
    // square control a true circle.
    Binding {
        target: root.background
        property: "radius"
        value: CelestinaTheme.radiusPill
    }

    // Inset from the control's own bounds so the circle sits *inside* the
    // reading capsule that carries it. Flush, its lower edge coincided with
    // the welded capsule's, and hovering an icon read as a shape colliding
    // with the pill rather than resting in it. `Control` insets move the
    // background alone: the icon, the hit area and every layout number around
    // this control are untouched.
    topInset: CelestinaTheme.spaceXs
    bottomInset: CelestinaTheme.spaceXs
    leftInset: CelestinaTheme.spaceXs
    rightInset: CelestinaTheme.spaceXs
    holdHoverFeedback: root.menuOpen
    leftPadding: 0
    rightPadding: 0
    topPadding: 0
    bottomPadding: 0
    activeFocusOnTab: true

    // On the press, never on the click. A click completes on the release,
    // and that release can die: with a contextual surface holding on-demand
    // keyboard focus, the press on the bar makes the compositor pull that
    // focus away mid-gesture — measured on the nested session as a
    // `wl_keyboard.leave` in the very batch of the press — and Qt answers
    // the focus loss by cancelling the button's grab, so `clicked` never
    // fires and the first click on any opener silently dies while a menu is
    // up. The press is delivered before any of that can happen, and a bar
    // that answers on the press is also simply faster in the hand.
    onPressed: {
        if (root.openedByHover && root.menuOpen) {
            root.openedByHover = false;
            return;
        }
        root.requestMenu();
    }
    Keys.onReturnPressed: function(event) {
        root.requestMenu();
        event.accepted = true;
    }
    Keys.onEnterPressed: function(event) {
        root.requestMenu();
        event.accepted = true;
    }

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.NoButton
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
    }

    // The dwell that separates resting on a control from crossing it. It is
    // restarted rather than merely started on every hover, so a pointer that
    // wanders in and out never accumulates its way to an opening.
    Timer {
        id: hoverDwell

        // Short enough to read as immediate, long enough that a pointer
        // crossing the bar on its way somewhere else never opens anything.
        interval: CelestinaTheme.motionFast
        onTriggered: {
            if (root.opensOnHover && root.hovered && !root.menuOpen) {
                root.openedByHover = true;
                root.requestMenu();
            }
        }
    }

    // Whatever is up stopped being this control's the moment the lease let go,
    // so the next click is an ordinary open again.
    onMenuOpenChanged: {
        if (!root.menuOpen)
            root.openedByHover = false;
    }

    onHoveredChanged: {
        if (!root.opensOnHover)
            return;
        // An opener whose own surface is already up has nothing to ask for;
        // asking again would retire and remap the very menu being reached for.
        if (root.hovered && !root.menuOpen)
            hoverDwell.restart();
        else
            hoverDwell.stop();
    }
}
