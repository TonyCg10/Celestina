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
        implicitWidth: CelestinaTheme.compSwitchTrackWidth
        implicitHeight: CelestinaTheme.compSwitchTrackHeight
        radius: height / 2
        // Off: a neutral control wash; on: the accent. Focus lifts a ring.
        color: control.checked ? CelestinaTheme.accent : CelestinaTheme.controlFill
        border.width: CelestinaTheme.borderHairline
        border.color: control.checked ? CelestinaTheme.accent
                      : CelestinaTheme.divider
        opacity: control.enabled ? 1 : CelestinaTheme.disabledOpacity

        CelestinaFocusRing {
            target: parent
            cornerRadius: parent.radius
            shown: control.visualFocus
        }

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
            }
        }

        Rectangle {
            id: thumb
            width: CelestinaTheme.compSwitchThumbSize
            height: CelestinaTheme.compSwitchThumbSize
            radius: height / 2
            // The thumb is always the light disc (white in both states, One UI);
            // the track carries the state.
            color: CelestinaTheme.text
            anchors.verticalCenter: parent.verticalCenter
            x: control.checked
               ? parent.width - width - CelestinaTheme.compSwitchThumbInset
               : CelestinaTheme.compSwitchThumbInset

            Behavior on x {
                NumberAnimation {
                    duration: CelestinaTheme.reducedMotion
                              ? 0 : CelestinaTheme.motionNormal
                    easing.type: CelestinaTheme.easeStandard
                }
            }
        }
    }
}
