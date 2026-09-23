import QtQuick
import org.celestina.siderita 1.0

// Shared wheel policy for folder views.
//
// Two jobs, and they no longer fight each other. Traditional wheels emit
// discrete notches, so their destination is accumulated and tweened; touchpads
// already deliver a smooth stream and are applied straight through. And every
// gesture, whichever kind, also moves the heading's travel.
//
// It used to be three transitions with a detent each, and crossing one consumed
// the whole gesture: the notch that folded the heading did not scroll, and
// neither did the one that retired it. Scrolling and the heading now share the
// same delta, so nothing is ever spent twice or lost.
WheelHandler {
    id: root

    required property Flickable view
    // How far the heading has been scrolled away. Injected rather than mirrored
    // in properties here: one owner for the number, and this handler is only
    // one of the things that may move it.
    required property var heading
    property real targetContentY: 0

    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad

    function minimumY() {
        return view.originY - view.topMargin
    }

    function maximumY() {
        return Math.max(minimumY(),
                        view.originY + view.contentHeight - view.height)
    }

    function boundedY(value) {
        return Math.max(minimumY(), Math.min(maximumY(), value))
    }

    // Where the listing itself ends. The top margin above it is the room the
    // metadata block grows into, so the heading starts as soon as the rows are
    // fully shown rather than at the margin's far side — gating on the far side
    // made the whole margin dead scroll, rising into an empty band while the
    // heading did nothing.
    function atContentTop() {
        return view.contentY <= view.originY + 0.5
    }

    function resetTarget() {
        wheelAnimation.stop()
        targetContentY = boundedY(view.contentY)
        if (Math.abs(view.contentY - targetContentY) > 0.01)
            view.contentY = targetContentY
    }

    // Geometry moved under the gesture: the heading returning lifts the bar,
    // which lifts the content frame, which changes this list's top margin — and
    // it does that on every frame of the return. Cancelling the gesture there
    // is what made the scroll stop dead while the heading animated.
    //
    // So it intervenes only when the destination has actually stopped being
    // legal, which a growing top margin never does; the rest of the time a
    // gesture still in flight is left alone to finish delivering.
    function rebound() {
        const bounded = boundedY(targetContentY)
        if (Math.abs(bounded - targetContentY) < 0.01)
            return
        targetContentY = bounded
        if (wheelAnimation.running) {
            wheelAnimation.stop()
            wheelAnimation.from = view.contentY
            wheelAnimation.to = bounded
            wheelAnimation.start()
            return
        }
        if (Math.abs(view.contentY - bounded) > 0.01)
            view.contentY = bounded
    }

    Component.onCompleted: resetTarget()
    onActiveChanged: {
        if (active && !wheelAnimation.running)
            targetContentY = boundedY(view.contentY)
    }

    property Connections viewConnections: Connections {
        target: root.view

        function onContentHeightChanged() { root.rebound() }
        function onHeightChanged() { root.rebound() }
        function onTopMarginChanged() { root.rebound() }
    }

    property NumberAnimation wheelAnimation: NumberAnimation {
        id: wheelAnimation
        target: root.view
        property: "contentY"
        duration: CelestinaTheme.motionNormal
        easing.type: CelestinaTheme.easeStandard
        onFinished: root.targetContentY = root.boundedY(root.view.contentY)
    }

    onWheel: function(event) {
        const pixelBased = Math.abs(event.pixelDelta.y) >= 0.01
        var delta = pixelBased ? event.pixelDelta.y : 0
        if (!pixelBased)
            delta = event.angleDelta.y / 120 * CelestinaTheme.compWheelStep
        if (Math.abs(delta) < 0.01)
            return

        // Asked before the listing moves: arriving at the top during this
        // gesture must not also grow the metadata block. That still takes a
        // second push — the one rule the old state machine had that is worth
        // keeping — but the gesture that arrives is no longer swallowed.
        root.heading.advance(-delta, atContentTop(), !pixelBased)

        if (pixelBased) {
            // Touchpads already deliver a smooth stream of pixel deltas. A
            // second tween here would add latency and make the gesture gummy.
            wheelAnimation.stop()
            targetContentY = boundedY(view.contentY - delta)
            view.contentY = targetContentY
        } else {
            // Accumulate the destination while the previous tween is still
            // settling so rapid wheel input never loses distance.
            if (!wheelAnimation.running)
                targetContentY = boundedY(view.contentY)
            targetContentY = boundedY(targetContentY - delta)
            wheelAnimation.stop()
            wheelAnimation.from = view.contentY
            wheelAnimation.to = targetContentY
            wheelAnimation.start()
        }
        event.accepted = true
    }
}
