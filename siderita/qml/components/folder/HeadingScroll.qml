import QtQuick
import org.celestina.siderita 1.0

// How far the heading has been scrolled away, as one number.
//
// It replaces a state machine with three states and a detent per transition.
// Each of those detents consumed the gesture that crossed it — the notch that
// folded the heading did not also scroll the listing, and neither did the one
// that retired it — which is what read as needing a second scroll to catch up.
//
// Travel runs from `-expandSpan` (the metadata block fully open) through zero
// (the compact title, and the resting position) to `+retireSpan` (nothing but
// the listing). The two progresses the view interpolates on are a plain
// function of it, so there is no state to be in and nothing to cross.
//
// It is fed by the gesture rather than by `contentY` on purpose: the list's top
// margin follows `compactProgress`, and a heading that read the position it
// helps decide would close a binding loop.
QtObject {
    id: root

    // How much upward travel grows the metadata block. A scroll distance, not
    // the block's height: tying it to the height made the whole phase shorter
    // than one wheel notch (56px against 108), so a single notch blew through
    // it, started retiring the title with what was left, and dragged the
    // listing a full notch on the way. It is also why a folder without dates
    // behaved differently from one with them.
    required property real expandSpan
    // And how much downward travel takes the compact title away.
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
    // They differ only while a tween is running.
    property real travel: 0
    property real destination: 0

    readonly property real compactProgress:
            root.expandSpan > 0
            ? Math.max(0, Math.min(1, 1 + root.travel / root.expandSpan))
            : 1
    readonly property real retiredProgress:
            root.retireSpan > 0
            ? Math.max(0, Math.min(1, root.travel / root.retireSpan))
            : 0
    // Where the travel may run to: past the fade, into the credit above.
    readonly property real travelCeiling: root.retireSpan + root.returnDelay

    // A notch of a wheel is a jump, and the listing that notch scrolls glides
    // at `wheelVelocity` — so this glides at the same speed, or the two arrive
    // at different times and the heading reads as snapping while the rows
    // move. The glide is a Behavior with a SmoothedAnimation, not a restarted
    // NumberAnimation: notches arrive faster than a 200 ms tween settles, and
    // restarting an eased tween on every notch made the speed jump and brake
    // each time, which is the stutter the author recorded on a fast scroll. A
    // Behavior re-aims the running animation and carries its velocity over,
    // so a burst of notches is one continuous motion. A touchpad already
    // delivers a smooth stream and is applied straight through (`gliding`
    // off).
    //
    // The speed: one notch of the listing in `motionNormal`, as before, with
    // the easing capped at that same time so a long run never crawls.
    readonly property real wheelVelocity:
            CelestinaTheme.compWheelStep * 1000 / CelestinaTheme.motionNormal
    //
    // `Immediate`, not `Sync`: Sync snaps to the target the moment the
    // direction reverses — and the velocity it compares against survives a
    // stopped glide, so the first notch after an instant move could land at
    // once. A reversal here turns straight around from a standstill.
    property bool gliding: false
    Behavior on travel {
        enabled: root.gliding
        SmoothedAnimation {
            id: glide
            reversingMode: SmoothedAnimation.Immediate
            velocity: root.wheelVelocity
            maximumEasingTime: CelestinaTheme.motionNormal
        }
    }
    // Arrival, not `running`: a Behavior restarts its animation on every
    // re-aim, and `running` flips false and true inside that restart.
    onTravelChanged: if (root.gliding && root.travel === root.destination)
                         root.gliding = false

    // Moves the heading by a gesture's worth. Positive takes it away.
    //
    // The amount is added to the destination, never to the drawn value: notches
    // arrive faster than the tween settles, and accumulating on what is on
    // screen would throw away everything the previous notch had not yet spent —
    // the same reason the wheel handler keeps its own `targetContentY`.
    //
    // `atTop` is the one rule left from the old machine: growing the metadata
    // block is still something asked for at the top of the listing, so away
    // from it the travel stops at the compact title instead of going past it.
    function advance(amount, atTop, smoothed) {
        const floor = atTop ? -root.expandSpan : 0
        root.moveTo(Math.max(floor, Math.min(root.travelCeiling,
                                             root.destination + amount)),
                    smoothed)
    }

    // Puts the metadata block away without bringing back a title that was
    // scrolled off: changing mode or folder should not undo that. Clamped on
    // the destination, like every other move.
    function fold() {
        root.moveTo(Math.max(0, root.destination), true)
    }

    // Puts the heading somewhere outright: a change of folder or of view mode,
    // which is a context change rather than a gesture.
    function moveTo(value, smoothed) {
        root.destination = value
        // A disabled Behavior writes the value directly and stops a glide
        // still in flight (the animation cannot be stopped from outside: a
        // `stop()` on it is refused as a non-root animation node).
        if (smoothed === false || CelestinaTheme.reducedMotion) {
            root.gliding = false
            root.travel = value
            return
        }
        root.gliding = true
        root.travel = value
    }
}
