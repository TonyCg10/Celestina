import QtQuick

// How far the heading has been scrolled away, as one number.
//
// It replaces a state machine with three states and a detent per transition.
// Each of those detents consumed the gesture that crossed it — the notch that
// folded the heading did not also scroll the listing, and neither did the one
// that retired it — which is what read as needing a second scroll to catch up.
//
// Travel runs from `-expandedExtra` (the metadata block fully open) through
// zero (the compact title, and the resting position) to `+retireSpan` (nothing
// but the listing). The two progresses the view interpolates on are a plain
// function of it, so there is no state to be in and nothing to cross.
//
// It is fed by the gesture rather than by `contentY` on purpose: the list's top
// margin follows `compactProgress`, and a heading that read the position it
// helps decide would close a binding loop.
QtObject {
    id: root

    // The metadata block's own height — how far above the compact title the
    // travel may go. The heading publishes it; it changes with the folder,
    // because a location with no dates is a shorter block.
    required property real expandedExtra
    // How much downward travel takes the compact title away. Sized from the
    // wheel step so it stays the distance a person already had to scroll.
    required property real retireSpan

    property real travel: 0

    readonly property real compactProgress:
            root.expandedExtra > 0
            ? Math.max(0, Math.min(1, 1 + root.travel / root.expandedExtra))
            : 1
    readonly property real retiredProgress:
            root.retireSpan > 0
            ? Math.max(0, Math.min(1, root.travel / root.retireSpan))
            : 0

    // Moves the heading by a gesture's worth. Positive takes it away.
    //
    // `atTop` is the one rule left from the old machine: growing the metadata
    // block is still something asked for at the top of the listing, so away
    // from it the travel stops at the compact title instead of going past it.
    function advance(amount, atTop) {
        const floor = atTop ? -root.expandedExtra : 0
        root.travel = Math.max(floor, Math.min(root.retireSpan,
                                               root.travel + amount))
    }
}
