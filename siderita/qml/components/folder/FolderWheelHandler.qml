import QtQuick
import org.celestina.siderita 1.0

// Shared wheel policy for folder views. Reaching the top and asking to reveal
// the heading are deliberately separate gestures: a continuous touchpad swipe
// that merely arrives at the boundary cannot open the expanded header.
WheelHandler {
    id: root

    required property Flickable view
    property bool revealArmed: false
    property real targetContentY: 0

    signal revealRequested
    signal collapseRequested

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

    function maybeArmReveal() {
        if (!root.active && !wheelAnimation.running)
            revealArmed = view.atYBeginning
                          || view.contentY <= minimumY() + 0.5
    }

    function resetTarget() {
        wheelAnimation.stop()
        targetContentY = boundedY(view.contentY)
        if (Math.abs(view.contentY - targetContentY) > 0.01)
            view.contentY = targetContentY
        maybeArmReveal()
    }

    Component.onCompleted: resetTarget()
    onActiveChanged: {
        if (active && !wheelAnimation.running)
            targetContentY = boundedY(view.contentY)
        else if (!active)
            maybeArmReveal()
    }

    property Connections viewConnections: Connections {
        target: root.view

        function onContentHeightChanged() { root.resetTarget() }
        function onHeightChanged() { root.resetTarget() }
        function onTopMarginChanged() { root.resetTarget() }
    }

    property NumberAnimation wheelAnimation: NumberAnimation {
        id: wheelAnimation
        target: root.view
        property: "contentY"
        duration: CelestinaTheme.motionNormal
        easing.type: CelestinaTheme.easeStandard
        onFinished: {
            root.targetContentY = root.boundedY(root.view.contentY)
            root.maybeArmReveal()
        }
    }

    onWheel: function(event) {
        const pixelBased = Math.abs(event.pixelDelta.y) >= 0.01
        var delta = pixelBased ? event.pixelDelta.y : 0
        if (!pixelBased)
            delta = event.angleDelta.y / 120 * CelestinaTheme.compWheelStep
        if (Math.abs(delta) < 0.01)
            return

        const wasAtBeginning = view.atYBeginning
                               || view.contentY <= minimumY() + 0.5

        if (delta > 0 && wasAtBeginning && revealArmed) {
            wheelAnimation.stop()
            targetContentY = boundedY(view.contentY)
            revealArmed = false
            root.revealRequested()
        } else if (delta < 0) {
            revealArmed = false
            root.collapseRequested()
        }

        if (pixelBased) {
            // Touchpads already deliver a smooth stream of pixel deltas. A
            // second tween here would add latency and make the gesture gummy.
            wheelAnimation.stop()
            targetContentY = boundedY(view.contentY - delta)
            view.contentY = targetContentY
        } else {
            // Traditional wheels emit discrete notches. Accumulate their
            // destination while the previous tween is still settling so rapid
            // wheel input never loses distance.
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
