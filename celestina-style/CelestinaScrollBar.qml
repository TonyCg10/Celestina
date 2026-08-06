import QtQuick

// A Flickable's scroll position: how much of the content is on screen, where
// in it you are, and a handle to drag.
//
// Built from primitives rather than from a re-skinned `QtQuick.Controls`
// `ScrollBar`, so the suite owns its whole anatomy — thickness, resting and
// active colour, and the motion between them — from semantic tokens instead of
// from a control template that would have to be fought for each of them.
//
// It has no keyboard of its own on purpose. Everything it can do — line, page,
// start and end — the surface it reports on already does from the keyboard,
// and a scroll bar that competed for Tab would put a stop between that surface
// and the next control for no new capability. Hosts must therefore keep their
// content keyboard-reachable; this is an affordance, not the only way through.
Item {
    id: root

    // The viewport being reported. Read for geometry and moved by dragging;
    // nothing else about it is this component's business.
    required property Flickable surface
    // Vertical by default, because that is the axis a document always has.
    property bool horizontal: false

    readonly property int restingThickness: CelestinaTheme.spaceXs
    readonly property int activeThickness: CelestinaTheme.spaceSm
    // Short enough to stay out of the way, long enough to still be grabbed in
    // a document where the visible fraction rounds to nothing.
    readonly property int minimumHandle: CelestinaTheme.space2xl

    // Only the visible fraction is read from `visibleArea`: it answers "how
    // much of this is on screen", which is a length question. "How far down are
    // we" is answered below in content coordinates instead, because that is the
    // unit the drag writes back in.
    readonly property real shownFraction: root.horizontal
        ? root.surface.visibleArea.widthRatio : root.surface.visibleArea.heightRatio

    readonly property real trackLength: root.horizontal ? root.width : root.height
    readonly property real handleLength: Math.max(
        root.minimumHandle, Math.min(root.trackLength, root.shownFraction * root.trackLength))
    // What the handle can travel, and what the content can travel, are two
    // different distances; dragging converts between them.
    readonly property real handleTravel: Math.max(0, root.trackLength - root.handleLength)
    readonly property real contentTravel: root.horizontal
        ? Math.max(0, root.surface.contentWidth - root.surface.width)
        : Math.max(0, root.surface.contentHeight - root.surface.height)

    // How far the content has actually been scrolled, in the coordinates
    // `scrollToHandle` writes back.
    readonly property real contentOffset: root.horizontal
        ? root.surface.contentX : root.surface.contentY

    // Resting position and drag are one mapping read in both directions: the
    // handle sits at the same fraction of `handleTravel` that the content sits
    // at of `contentTravel`, which is exactly what `scrollToHandle` inverts.
    //
    // Scaling the scrolled fraction by the whole track instead would only agree
    // with that while the handle is exactly its proportional length, and the
    // `minimumHandle` clamp breaks the equality in precisely the long documents
    // it exists for: the handle would then hit the end of the track with
    // document still to read, and run ahead of the pointer while dragged.
    readonly property real handleOffset: root.contentTravel <= 0
        ? 0
        : Math.max(0, Math.min(root.handleTravel,
                               root.contentOffset / root.contentTravel * root.handleTravel))

    // Nothing to scroll, nothing to say. A bar pinned at full length would be
    // a permanent line down the edge of the text carrying no information.
    //
    // Just short of one rather than one: content exactly as wide as its
    // viewport — wrapped text, always — divides to a ratio that can land a
    // hair under 1, and that hair would otherwise be a visible bar with a
    // full-length handle that cannot move.
    visible: root.shownFraction > 0 && root.shownFraction < 0.999

    implicitWidth: root.horizontal ? 0 : root.activeThickness
    implicitHeight: root.horizontal ? root.activeThickness : 0

    Accessible.role: Accessible.ScrollBar
    Accessible.name: root.horizontal ? "Horizontal scroll" : "Vertical scroll"

    /// Moves the viewport so the handle starts at `offset` along the track.
    function scrollToHandle(offset) {
        if (root.handleTravel <= 0)
            return
        const reached = Math.max(0, Math.min(root.handleTravel, offset))
        const content = reached / root.handleTravel * root.contentTravel
        if (root.horizontal)
            root.surface.contentX = content
        else
            root.surface.contentY = content
    }

    Rectangle {
        id: handle

        x: root.horizontal ? root.handleOffset : (root.width - width) / 2
        y: root.horizontal ? (root.height - height) / 2 : root.handleOffset
        width: root.horizontal ? root.handleLength : root.thickness
        height: root.horizontal ? root.thickness : root.handleLength
        radius: Math.min(width, height) / 2
        color: track.pressed ? CelestinaTheme.textMuted : CelestinaTheme.inputBorder

        // The bar thickens under the pointer: at rest it is a hairline beside
        // the text, and it only becomes a target once one is wanted.
        Behavior on width {
            enabled: !CelestinaTheme.reducedMotion
            NumberAnimation { duration: CelestinaTheme.motionFast }
        }
        Behavior on height {
            enabled: !CelestinaTheme.reducedMotion
            NumberAnimation { duration: CelestinaTheme.motionFast }
        }
        Behavior on color {
            enabled: !CelestinaTheme.reducedMotion
            ColorAnimation { duration: CelestinaTheme.motionFast }
        }
    }

    readonly property int thickness: track.containsMouse || track.pressed
                                     ? root.activeThickness : root.restingThickness

    // One area over the whole track, not one over the moving handle: a grab
    // measured against something that moves as you drag it feeds back into
    // itself and stutters. Track coordinates hold still.
    MouseArea {
        id: track
        anchors.fill: parent
        hoverEnabled: true
        preventStealing: true

        // Where inside the handle the drag started, so it does not jump under
        // the pointer on the first pixel of movement.
        property real grabWithinHandle: 0

        function pointOf(mouse) {
            return root.horizontal ? mouse.x : mouse.y
        }

        onPressed: function(mouse) {
            const point = track.pointOf(mouse)
            const start = root.handleOffset
            if (point >= start && point <= start + root.handleLength) {
                track.grabWithinHandle = point - start
                return
            }
            // Pressing the empty track jumps the handle's centre there, which
            // is what a click ahead of the handle is asking for.
            track.grabWithinHandle = root.handleLength / 2
            root.scrollToHandle(point - track.grabWithinHandle)
        }

        onPositionChanged: function(mouse) {
            if (!track.pressed)
                return
            root.scrollToHandle(track.pointOf(mouse) - track.grabWithinHandle)
        }
    }
}
