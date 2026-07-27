import QtQuick
import QtQuick.Controls

// ─── CelestinaSwitch ──────────────────────────────────────────────────────────
// The One UI toggle: a pill track with a white thumb that slides, the track
// turning accent-blue when on (DESIGN §6.8). A real Switch underneath, so it
// keeps keyboard, focus and accessibility; only the indicator is restyled. No
// label — the row it sits in carries the text.
// ──────────────────────────────────────────────────────────────────────────────
Switch {
    id: control

    implicitWidth: track.implicitWidth
    implicitHeight: track.implicitHeight
    padding: 0

    indicator: Rectangle {
        id: track
        implicitWidth: 44
        implicitHeight: 26
        radius: height / 2
        // Off: a neutral control wash; on: the accent. Focus lifts a ring.
        color: control.checked ? CelestinaTheme.accent : CelestinaTheme.controlFill
        border.width: control.activeFocus ? 2 : 1
        border.color: control.activeFocus ? CelestinaTheme.focusRing
                      : control.checked ? CelestinaTheme.accent
                      : CelestinaTheme.divider
        opacity: control.enabled ? 1 : 0.5

        Behavior on color {
            ColorAnimation { duration: CelestinaTheme.motionFast }
        }

        Rectangle {
            id: thumb
            width: 20
            height: 20
            radius: height / 2
            // The thumb is always the light disc (white in both states, One UI);
            // the track carries the state.
            color: CelestinaTheme.text
            anchors.verticalCenter: parent.verticalCenter
            x: control.checked ? parent.width - width - 3 : 3

            Behavior on x {
                NumberAnimation {
                    duration: CelestinaTheme.motionNormal
                    easing.type: CelestinaTheme.easeStandard
                }
            }
        }
    }
}
