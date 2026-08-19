pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Shapes
import org.celestina.siderita 1.0

    // ── What one operation is doing ───────────────────────────────────
    // The card a ring opens: the action, the entry it has reached, how much has
    // moved, and the Cancel that stops this job and no other. It points at the
    // ring that opened it, so with several running there is no doubt which one
    // is being talked about.
Item {
    id: callout

    property var controller
    property Item backdrop
    // The job this is about; empty means nothing is open.
    property string jobId: ""
    property int jobIndex: -1
    // Hundredths, or below zero when there is no knowable fraction.
    property int percent: -1
    // Whether this job is being held, which changes one button's face and the
    // line above it.
    property bool paused: false
    // Where the pointer should sit, in the parent's coordinates.
    property real pointerX: 0

    signal dismissed()

    readonly property int pointerSize: 14

    function at(list, index) {
        return list !== undefined && index >= 0 && index < list.length ? list[index] : ""
    }

    visible: callout.jobId.length > 0 && callout.jobIndex >= 0
    // A fixed width, not one derived from the text: sizing the card from its
    // content while the content is sized from the card is a cycle, and QML
    // answers a cycle by laying nothing out properly. The lines elide instead.
    implicitWidth: 264
    implicitHeight: card.height + callout.pointerSize * 0.62
    opacity: callout.visible ? 1 : 0
    Behavior on opacity {
        NumberAnimation { duration: CelestinaTheme.motionFast }
    }

    GlassCard {
        id: card
        width: parent.width
        height: body.implicitHeight + 24
        backdropSource: callout.backdrop

        // The card is a surface of its own: a press on it belongs to it, not to
        // the row of files underneath.
        CelestinaInputShield { }

        Column {
            id: body
            x: 14
            y: 12
            width: parent.width - 28
            spacing: 3

            Text {
                width: parent.width
                text: callout.at(callout.controller.opLabels, callout.jobIndex)
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                font.weight: CelestinaTheme.weightDemiBold
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                text: callout.at(callout.controller.opCurrents, callout.jobIndex)
                visible: text.length > 0
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                elide: Text.ElideMiddle
            }

            Text {
                width: parent.width
                text: callout.at(callout.controller.opDetails, callout.jobIndex)
                visible: text.length > 0
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                elide: Text.ElideRight
            }

            Item { width: 1; height: 4 }

            // The bar belongs here rather than on the ring: a ring shows that
            // something is happening, a bar shows how much is left, and only the
            // callout has the room to be read as a measurement.
            Rectangle {
                objectName: "calloutTrack"
                width: parent.width
                height: CelestinaTheme.compLinearTrackHeight
                radius: height / 2
                color: CelestinaTheme.controlFill
                visible: callout.percent >= 0

                Rectangle {
                    height: parent.height
                    radius: height / 2
                    color: CelestinaTheme.accent
                    width: parent.width * Math.min(1, Math.max(0, callout.percent) / 100)
                    // Reports arrive at most every 60 ms; this carries the bar
                    // between two of them so it reads as movement rather than
                    // as a series of jumps.
                    Behavior on width {
                        NumberAnimation { duration: CelestinaTheme.motionFast }
                    }
                }
            }

            Item { width: 1; height: 8 }

            // Held or running, then stopped for good: the two answers a person
            // can give a long operation, in the order they are reached for.
            Row {
                anchors.right: parent.right
                spacing: 8

                CelestinaButton {
                    objectName: "calloutPause"
                    height: 28
                    text: callout.paused ? qsTr("Reanudar") : qsTr("Pausar")
                    Accessible.name: (callout.paused ? qsTr("Reanudar %1") : qsTr("Pausar %1"))
                        .arg(callout.at(callout.controller.opLabels, callout.jobIndex))
                    // Pausing leaves the callout open: a person who holds a copy
                    // usually wants to watch that it really stopped.
                    onClicked: callout.controller.toggleJobPaused(
                                   parseFloat(callout.jobId))
                }

                CelestinaButton {
                    objectName: "calloutCancel"
                    height: 28
                    text: qsTr("Cancelar")
                    Accessible.name: qsTr("Cancelar %1").arg(
                        callout.at(callout.controller.opLabels, callout.jobIndex))
                    onClicked: {
                        callout.controller.cancelJob(parseFloat(callout.jobId))
                        callout.dismissed()
                    }
                }
            }
        }
    }

    // The pointer: a real triangle drawn pointing *down*, at the ring, and
    // overlapping the card by a pixel so the two read as one shape. It was a
    // rotated square before, which at this size read as a loose diamond that
    // pointed nowhere in particular.
    Shape {
        objectName: "calloutPointer"
        // `pointerX` and `callout.x` are both in the dock's coordinates, so the
        // difference is this card's own; the clamp only keeps the tip from
        // hanging off a rounded corner.
        x: Math.max(10, Math.min(callout.width - 10 - width,
                                 callout.pointerX - callout.x - width / 2))
        y: card.height - 1
        width: callout.pointerSize
        height: callout.pointerSize * 0.62
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            fillColor: CelestinaTheme.card
            strokeColor: CelestinaTheme.clear
            strokeWidth: 0
            startX: 0
            startY: 0
            PathLine { x: callout.pointerSize; y: 0 }
            PathLine { x: callout.pointerSize / 2; y: callout.pointerSize * 0.62 }
            PathLine { x: 0; y: 0 }
        }
    }
}
