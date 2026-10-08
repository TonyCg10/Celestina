import QtQuick
import org.celestina.cuprita 1.0

// A volume slider in whole percentages: 0 to 150 % in steps of 1 %, with a
// tick at 100 % where boosting begins. `requested` carries the percentage to
// write: at once, then at most once per 100 ms while it moves, and always the
// final value when a drag ends, so a drag never floods the sound server.
CelestinaSlider {
    id: slider

    to: 150
    step: 1

    signal requested(real value)

    // The newest value not yet requested, or -1.
    property real unsent: -1

    function flush() {
        if (slider.unsent >= 0) {
            slider.requested(slider.unsent)
            slider.unsent = -1
        }
    }

    onMoved: function(value) {
        slider.unsent = value
        if (!throttle.running) {
            slider.flush()
            throttle.start()
        }
    }
    onDraggingChanged: {
        if (!slider.dragging)
            slider.flush()
    }

    // Shift with an arrow moves 5 %. The style's slider handles the arrow in
    // its own Keys handler, which runs before any handler here and accepts
    // the key, so `Keys.priority` cannot put this one first. Instead a key
    // step is held until the event is over: the style's +1 % lands in
    // `keyTarget`, the Shift handler below (same event) replaces it with the
    // 5 % target, and one value is emitted.
    property real keyTarget: -1

    function moveTo(target) {
        if (!slider.enabled || slider.span <= 0)
            return
        slider.keyTarget = Math.max(slider.from, Math.min(slider.to, target))
        Qt.callLater(slider.emitKeyTarget)
    }

    function emitKeyTarget() {
        if (slider.keyTarget < 0)
            return
        const target = slider.keyTarget
        slider.keyTarget = -1
        slider.moved(target)
    }

    Keys.onLeftPressed: function(event) {
        if (event.modifiers & Qt.ShiftModifier)
            slider.moveTo(slider.value - 5)
    }
    Keys.onRightPressed: function(event) {
        if (event.modifiers & Qt.ShiftModifier)
            slider.moveTo(slider.value + 5)
    }

    Timer {
        id: throttle
        // The write rate, not an animation: `motionFast` is the suite's 100 ms.
        interval: CelestinaTheme.motionFast
        onTriggered: slider.flush()
    }

    // The tick sits under the track and the thumb; it shows above and below
    // the hairline.
    Rectangle {
        objectName: "fullVolumeTick"
        z: -1
        width: CelestinaTheme.spaceXs
        height: CelestinaTheme.spaceMd
        radius: CelestinaTheme.radiusPill
        anchors.verticalCenter: parent.verticalCenter
        x: slider.handleSize / 2 + (slider.width - slider.handleSize) * 100 / slider.to - width / 2
        color: CelestinaTheme.divider
    }
}
