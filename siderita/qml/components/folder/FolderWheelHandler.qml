import QtQuick
import org.celestina.siderita 1.0

// Shared wheel policy for folder views.
//
// Two jobs, and they no longer fight each other. The listing scrolls the way
// every Celestina list does (`CelestinaWheelScroll`): a notch adds to a
// destination the rows follow by a fixed share of what is left every frame,
// so it shows on the next frame and a burst adds up exactly; touchpads
// already deliver a smooth stream and are applied straight through. And every
// gesture, whichever kind, also moves the heading's travel — the same delta,
// followed the same way, so the heading and the rows arrive together.
//
// It used to be three transitions with a detent each, and crossing one consumed
// the whole gesture: the notch that folded the heading did not scroll, and
// neither did the one that retired it. Scrolling and the heading now share the
// same delta, so nothing is ever spent twice or lost. `stepped` arrives before
// the listing moves and even when it cannot (a push up at the top), so the
// heading hears every gesture.
//
// The glide that used to live here (a SmoothedAnimation on a private value)
// started every notch from a standstill — two pixels on the first frame —
// which is the brake the author felt on the wheel.
CelestinaWheelScroll {
    id: root

    // How far the heading has been scrolled away. Injected rather than mirrored
    // in properties here: one owner for the number, and this handler is only
    // one of the things that may move it.
    required property var heading

    onStepped: function(delta, notch) { root.heading.advance(-delta, notch) }

    Component.onCompleted: root.retarget()

    // Geometry moved under the gesture: the heading returning lifts the bar,
    // which lifts the content frame, which may change this list's top margin —
    // on every frame of the return. Cancelling the gesture there is what made
    // the scroll stop dead while the heading animated.
    //
    // So `retarget()` only pulls the destination back inside the bounds, which
    // a growing top margin never moves it out of; a glide still in flight is
    // left alone to finish delivering.
    property Connections viewConnections: Connections {
        target: root.view

        function onContentHeightChanged() { root.retarget() }
        function onHeightChanged() { root.retarget() }
        function onTopMarginChanged() { root.retarget() }
    }
}
