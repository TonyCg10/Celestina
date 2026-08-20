import QtQuick
import org.celestina.siderita 1.0

// Shared wheel policy for folder views.
//
// The heading has three states, and each transition asks for a different
// gesture — which is the whole point: they must not be reachable by accident.
//
// - **Expanded → compact** on any downward scroll, immediately. This is the
//   long-standing behaviour: the metadata block gets out of the way the moment
//   a person starts reading the folder.
// - **Compact → retired** only after `retireThreshold` of downward travel. The
//   compact title is what a folder shows by default, so taking it away is a
//   deliberate ask, not something a nudge of the wheel does.
// - **Retired → compact** on reaching the top; **compact → expanded** on a
//   further push once there, which is why arriving and asking are still two
//   separate signals.
WheelHandler {
    id: root

    required property Flickable view
    // What the heading is doing, so a gesture that changes it is not also
    // spent scrolling. Collapsing already lifts the whole chrome — the bar
    // rises and the rows rise with it — and adding the wheel's own travel on
    // top is the "extra scroll" a person feels as the content jumping away.
    property bool headingExpanded: false
    property bool headingRetired: false
    property real targetContentY: 0
    // How much downward travel retires the compact heading.
    //
    // Expressed in wheel steps, not pixels, because one notch *is* the unit a
    // person feels: a threshold below a single step would retire it on a nudge.
    // One and a half steps means two notches, or a deliberate touchpad swipe.
    readonly property real retireThreshold: CelestinaTheme.compWheelStep * 1.5
    property real retireTravel: 0
    // And a shorter one before the expanded heading folds. It used to go on the
    // first pixel of movement, which made the window rearrange itself while a
    // person was only easing into the list; three quarters of a step is still
    // one notch of a wheel, but a touchpad has to mean it.
    readonly property real collapseThreshold: CelestinaTheme.compWheelStep * 0.75
    property real collapseTravel: 0
    property bool revealArmed: false

    signal revealRequested     // expand: a further push at the top
    signal restoreRequested    // bring the compact heading back: arriving is enough
    signal collapseRequested   // expanded → compact, at once
    signal retireRequested     // compact → gone, past the threshold

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

    function atTop() {
        return view.atYBeginning || view.contentY <= minimumY() + 0.5
    }

    function maybeArmReveal() {
        if (!root.active && !wheelAnimation.running)
            revealArmed = atTop()
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
            if (root.atTop())
                root.restoreRequested()
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

        if (delta > 0) {
            retireTravel = 0
            collapseTravel = 0
            if (atTop()) {
                // Arriving brings the compact heading back; a further push from
                // there — the armed gesture — expands it.
                root.restoreRequested()
                if (revealArmed) {
                    revealArmed = false
                    root.revealRequested()
                }
            }
        } else {
            revealArmed = false
            // The expanded heading always yields at once; only taking away the
            // compact one waits for a real gesture.
            if (root.headingExpanded) {
                collapseTravel += -delta
                if (collapseTravel >= collapseThreshold) {
                    collapseTravel = 0
                    root.collapseRequested()
                    event.accepted = true
                    return      // this gesture changed the heading; that is all
                }
                // Not far enough yet: let the listing scroll as usual.
            }
            retireTravel += -delta
            if (retireTravel >= retireThreshold) {
                retireTravel = 0
                root.retireRequested()
                event.accepted = true
                return
            }
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
