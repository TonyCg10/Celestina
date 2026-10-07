import QtQuick
import org.celestina.siderita 1.0

// How far the heading has been scrolled away, as one number.
//
// It replaces a state machine with three states and a detent per transition.
// Each of those detents consumed the gesture that crossed it — the notch that
// folded the heading did not also scroll the listing, and neither did the one
// that retired it — which is what read as needing a second scroll to catch up.
//
// Travel runs from zero (the title, and the resting position) to
// `+retireSpan` (nothing but the listing). The progress the view interpolates
// on is a plain function of it, so there is no state to be in and nothing to
// cross. There is no travel above zero: the metadata block that used to grow
// there on a push up at the top is gone (the author never reached for it, and
// its frame moving under the rows was where a fast scroll jumped), so a push
// up at the top is simply nothing.
//
// It is fed by the gesture rather than by `contentY` on purpose: a heading
// that read the position it helps decide would close a binding loop.
QtObject {
    id: root

    // How much downward travel takes the title away.
    required property real retireSpan
    // What the travel keeps accumulating once the title is already gone.
    //
    // Without it, one notch upward from deep inside a folder brings most of the
    // title back, which is what the author saw: the heading returning while the
    // listing was still hundreds of rows down. The credit is spent before the
    // title starts to return, so coming back is something asked for rather than
    // a side effect of easing upward — and it is bounded, so it never costs
    // more than these few notches however far down the person went.
    required property real returnDelay

    // Where the heading is, and where the gestures so far have asked it to be.
    // They differ only while the follower is closing on the destination.
    property real travel: 0
    property real destination: 0

    readonly property real retiredProgress:
            root.retireSpan > 0
            ? Math.max(0, Math.min(1, root.travel / root.retireSpan))
            : 0
    // Where the travel may run to: past the fade, into the credit above.
    readonly property real travelCeiling: root.retireSpan + root.returnDelay

    // A notch of a wheel is a jump, and the listing that notch scrolls follows
    // its destination by a fixed share of what is left every frame
    // (`CelestinaWheelScroll`, 1 - exp(-frame / wheelFollowMs)). The travel
    // follows its own destination by the same share, or the two arrive at
    // different times and the heading reads as snapping while the rows move.
    // A notch that lands while the previous one is still travelling only moves
    // the destination, so a burst of notches is one continuous motion, and the
    // first frame already shows a good part of the notch: the SmoothedAnimation
    // this replaces started every notch from a standstill, which read as a
    // brake. A touchpad already delivers a smooth stream and is applied
    // straight through, and so is anything under reduced motion.
    property FrameAnimation follower: FrameAnimation {
        onTriggered: {
            const remaining = root.destination - root.travel
            if (Math.abs(remaining) < 0.5) {
                root.travel = root.destination
                stop()
                return
            }
            root.travel += remaining
                    * (1 - Math.exp(-frameTime * 1000 / CelestinaTheme.wheelFollowMs))
        }
    }

    // Moves the heading by a gesture's worth. Positive takes it away; the
    // travel never goes above zero, the title at rest.
    //
    // The amount is added to the destination, never to the drawn value: notches
    // arrive faster than the follower settles, and accumulating on what is on
    // screen would throw away everything the previous notch had not yet spent —
    // the same reason the wheel handler keeps its own `targetY`.
    function advance(amount, smoothed) {
        root.moveTo(Math.max(0, Math.min(root.travelCeiling,
                                         root.destination + amount)),
                    smoothed)
    }

    // Settles a move in flight without bringing back a title that was
    // scrolled off: changing mode or folder should not undo that. Clamped on
    // the destination, like every other move.
    function fold() {
        root.moveTo(Math.max(0, root.destination), true)
    }

    // Puts the heading somewhere outright: a change of folder or of view mode,
    // which is a context change rather than a gesture.
    function moveTo(value, smoothed) {
        root.destination = value
        // An outright move also ends a follow still in flight, so nothing
        // drifts back toward an older destination afterwards.
        if (smoothed === false || CelestinaTheme.reducedMotion) {
            root.follower.stop()
            root.travel = value
            return
        }
        root.follower.start()
    }
}
