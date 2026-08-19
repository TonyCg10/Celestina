pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Shapes
import org.celestina.siderita 1.0

    // ── One running operation, as a ring ──────────────────────────────
    // A circle of glass with the action's own icon at its centre and its
    // progress drawn around it. Two states, because only one of them can be
    // told the truth: a job whose entries are countable fills the ring, and a
    // job whose end nothing can predict — one archive being extracted — turns
    // instead, which says "working" without claiming to know how much is left.
    //
    // It is a button: pressing it is what opens the callout that names what it
    // is doing. Nothing about that callout lives here.
Item {
    id: ring

    property string iconName: ""
    // Hundredths, or below zero when there is no knowable fraction.
    property int percent: -1
    // How many times the job has reported progress. It is what turns a ring
    // whose end cannot be known, and it comes from the data on purpose: an
    // animation moves the render node, and a node moved that way never
    // repainted on the author's machine — the ring sat still through two
    // attempts. A step changes the arc's geometry, which cannot be skipped.
    property int steps: 0
    property bool active: false
    property alias hovered: hover.hovered

    readonly property bool indeterminate: ring.percent < 0
    readonly property real ringWidth: 3

    signal clicked()

    implicitWidth: 40
    implicitHeight: 40

    // The track the arc runs on, always fully drawn so the ring reads as a
    // control rather than as a fragment.
    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: CelestinaTheme.controlFill
            strokeWidth: ring.ringWidth
            fillColor: CelestinaTheme.clear
            capStyle: ShapePath.RoundCap
            PathAngleArc {
                centerX: ring.width / 2
                centerY: ring.height / 2
                radiusX: (ring.width - ring.ringWidth) / 2
                radiusY: (ring.height - ring.ringWidth) / 2
                startAngle: 0
                sweepAngle: 360
            }
        }
    }

    // Where the arc starts: twelve o'clock while it fills, and one step further
    // round on every report while it turns. The angle is left to grow past 360
    // on purpose — wrapping it would make the interpolation below run the long
    // way back around once per turn.
    readonly property real targetStart: ring.indeterminate ? -90 + ring.steps * 14 : -90
    // The drawn angle follows the target through the Behavior below, so the two
    // are separate: one is where the data says the arc is, the other is where it
    // is on screen right now.
    property real arcStart: ring.targetStart

    // Reports arrive at most every 60 ms, which on its own reads as a stutter.
    // Interpolating between two of them is what makes the ring move at the
    // screen's rate rather than at the worker's.
    Behavior on arcStart {
        enabled: ring.indeterminate
        NumberAnimation {
            duration: CelestinaTheme.motionFast
            easing.type: CelestinaTheme.easeStandard
        }
    }

    Shape {
        id: arc
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            strokeColor: CelestinaTheme.accent
            strokeWidth: ring.ringWidth
            fillColor: CelestinaTheme.clear
            capStyle: ShapePath.RoundCap
            PathAngleArc {
                centerX: ring.width / 2
                centerY: ring.height / 2
                radiusX: (ring.width - ring.ringWidth) / 2
                radiusY: (ring.height - ring.ringWidth) / 2
                startAngle: ring.arcStart
                sweepAngle: ring.indeterminate
                            ? 90
                            : 3.6 * Math.max(0, Math.min(100, ring.percent))
                Behavior on sweepAngle {
                    enabled: !ring.indeterminate
                    NumberAnimation { duration: CelestinaTheme.motionNormal }
                }
            }
        }
    }

    CelestinaIcon {
        anchors.centerIn: parent
        width: Math.round(parent.width * 0.45)
        height: width
        name: ring.iconName
        fallbackName: "file"
        tone: ring.active || ring.hovered ? CelestinaIcon.Accent
                                          : CelestinaIcon.Primary
    }

    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        onTapped: ring.clicked()
    }

    scale: hover.hovered ? 1.06 : 1
    Behavior on scale {
        NumberAnimation { duration: CelestinaTheme.motionFast }
    }
}
