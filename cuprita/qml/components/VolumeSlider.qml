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
