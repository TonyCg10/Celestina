pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.fluorita 1.0

// The span of a film a trim keeps: a track the length of the film, the kept
// part lit, and two handles, start and end.
//
// It owns no value. `start` and `end` come from the trim, which the domain
// checks; a handle that moves only asks, through `startMoved` and
// `endMoved`, and never further than the other handle less `minimumGap`, so
// the start cannot pass the end from the pointer or from the keyboard. The
// arrows move a handle by `step`, one frame of the film; with Shift, by a
// second; Home and End take it as far as it may go. Each handle is a slider
// to assistive technology, named for what it is and described by the time
// it stands on.
Item {
    id: bar

    required property real duration
    required property real start
    required property real end
    // The shortest span the trim accepts: the handles stay this far apart.
    property real minimumGap: 0
    // Where the picture is, drawn as a hairline; negative for nowhere.
    property real position: -1
    // What an arrow moves a handle by: one frame of the film, which the host
    // knows; one frame at 30 frames a second until it does.
    property real step: 1 / 30
    // What Shift and an arrow move it by.
    readonly property real largeStep: 1

    signal startMoved(real seconds)
    signal endMoved(real seconds)

    readonly property real handleWidth: CelestinaTheme.spaceMd
    // The film's time runs between the two handles' outer edges, so a handle
    // at either end still sits inside the bar.
    readonly property real trackX: bar.handleWidth
    readonly property real trackWidth: Math.max(1, bar.width - bar.handleWidth * 2)

    implicitHeight: CelestinaTheme.controlHeightSm

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Recorte")

    // `m:ss,mmm`: a cut is chosen to the frame, so a time is told to the
    // millisecond.
    function clock(seconds) {
        const whole = Math.max(0, seconds)
        const minutes = Math.floor(whole / 60)
        const rest = whole - minutes * 60
        const secondsPart = Math.floor(rest)
        const millis = Math.floor((rest - secondsPart) * 1000)
        return minutes + ":" + String(secondsPart).padStart(2, "0") + ","
            + String(millis).padStart(3, "0")
    }

    function xOf(seconds) {
        const share = bar.duration > 0 ? Math.max(0, Math.min(1, seconds / bar.duration)) : 0
        return bar.trackX + bar.trackWidth * share
    }

    function secondsAt(x) {
        return Math.max(0, Math.min(1, (x - bar.trackX) / bar.trackWidth)) * bar.duration
    }

    function moveStart(seconds) {
        if (!bar.enabled || bar.duration <= 0)
            return
        const target = Math.max(0, Math.min(seconds, bar.end - bar.minimumGap))
        if (target !== bar.start)
            bar.startMoved(target)
    }

    function moveEnd(seconds) {
        if (!bar.enabled || bar.duration <= 0)
            return
        const target = Math.min(bar.duration, Math.max(seconds, bar.start + bar.minimumGap))
        if (target !== bar.end)
            bar.endMoved(target)
    }

    Rectangle {
        id: track

        anchors.verticalCenter: parent.verticalCenter
        x: bar.trackX
        width: bar.trackWidth
        height: CelestinaTheme.spaceXs
        radius: CelestinaTheme.radiusPill
        color: CelestinaTheme.divider
    }

    // What is kept.
    Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        x: bar.xOf(bar.start)
        width: Math.max(0, bar.xOf(bar.end) - bar.xOf(bar.start))
        height: bar.height - CelestinaTheme.spaceSm
        radius: CelestinaTheme.radiusXs
        color: CelestinaTheme.accentSoft
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.accentSoftBorder
    }

    // Where the picture is.
    Rectangle {
        visible: bar.position >= 0 && bar.duration > 0
        anchors.verticalCenter: parent.verticalCenter
        x: bar.xOf(bar.position) - width / 2
        width: CelestinaTheme.spaceXs / 2
        height: bar.height
        radius: CelestinaTheme.radiusPill
        color: CelestinaTheme.text
    }

    // The two handles: the start sits just before the kept part, the end
    // just after it, so neither covers a frame that is kept.
    Repeater {
        model: 2

        delegate: Item {
            id: handle

            required property int index
            readonly property bool leading: handle.index === 0
            readonly property real value: handle.leading ? bar.start : bar.end
            // Where the pointer took hold of the handle, from its value's own
            // place, so a drag never jumps by half a handle.
            property real grip: 0
            // Qt says why focus arrived; a click must not raise the ring.
            property int focusReason: Qt.OtherFocusReason

            objectName: handle.leading ? "trimStart" : "trimEnd"
            x: handle.leading ? bar.xOf(bar.start) - bar.handleWidth : bar.xOf(bar.end)
            width: bar.handleWidth
            height: bar.height
            activeFocusOnTab: bar.enabled

            Accessible.role: Accessible.Slider
            Accessible.focusable: bar.enabled
            Accessible.name: handle.leading ? qsTr("Inicio del recorte") : qsTr("Final del recorte")
            Accessible.description: qsTr("%1. Flechas: un fotograma; con Mayúsculas, un segundo")
                .arg(bar.clock(handle.value))

            function moveTo(seconds) {
                if (handle.leading)
                    bar.moveStart(seconds)
                else
                    bar.moveEnd(seconds)
            }

            Rectangle {
                id: knob

                anchors.fill: parent
                radius: CelestinaTheme.radiusXs
                color: !bar.enabled ? CelestinaTheme.textFaint
                     : pointer.pressed ? CelestinaTheme.accentLink
                     : pointer.containsMouse ? CelestinaTheme.accentHover
                     : CelestinaTheme.accent

                Behavior on color {
                    ColorAnimation {
                        duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionFast
                    }
                }
            }

            CelestinaFocusRing {
                target: knob
                cornerRadius: knob.radius
                shown: handle.activeFocus && handle.focusReason !== Qt.MouseFocusReason
            }

            MouseArea {
                id: pointer

                anchors.fill: parent
                enabled: bar.enabled
                hoverEnabled: true
                preventStealing: true
                cursorShape: Qt.SizeHorCursor
                onPressed: function(mouse) {
                    handle.focusReason = Qt.MouseFocusReason
                    handle.forceActiveFocus(Qt.MouseFocusReason)
                    handle.grip = handle.mapToItem(bar, mouse.x, 0).x - bar.xOf(handle.value)
                }
                onPositionChanged: function(mouse) {
                    if (pointer.pressed)
                        handle.moveTo(bar.secondsAt(handle.mapToItem(bar, mouse.x, 0).x - handle.grip))
                }
            }

            onActiveFocusChanged: if (!handle.activeFocus) handle.focusReason = Qt.OtherFocusReason

            Keys.onPressed: function(event) {
                const amount = event.modifiers & Qt.ShiftModifier ? bar.largeStep : bar.step
                if (event.key === Qt.Key_Left || event.key === Qt.Key_Down) {
                    handle.moveTo(handle.value - amount)
                } else if (event.key === Qt.Key_Right || event.key === Qt.Key_Up) {
                    handle.moveTo(handle.value + amount)
                } else if (event.key === Qt.Key_Home) {
                    handle.moveTo(0)
                } else if (event.key === Qt.Key_End) {
                    handle.moveTo(bar.duration)
                } else {
                    return
                }
                event.accepted = true
            }
        }
    }
}
