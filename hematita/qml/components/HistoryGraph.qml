import QtQuick
import QtQuick.Shapes
import org.celestina.hematita 1.0

// The one graph in Hematita: a minute of a fraction, oldest at the left. A
// GPU-backed Shape draws a filled area and the trace over it from a series the
// adapter already normalised, so the page never does arithmetic on samples.
//
// Colour follows the resource's kind through theme tokens, and the load state
// overrides it; the fill is the same colour at the suite's soft opacity. The
// shape is drawn inside an inset, clipped plot so the fill never reaches the
// background's rounded corners.
Item {
    id: graph

    // Fractions 0..=1, oldest first. Any length; an empty series draws nothing.
    required property var series
    // "normal", "elevated" or "critical".
    required property string load
    // "cpu", "memory", "gpu", "disk" or "network".
    required property string kind

    readonly property color kindColor: {
        switch (graph.kind) {
        case "memory": return CelestinaTheme.glyphAccentViolet
        case "gpu": return CelestinaTheme.glyphAccentCoral
        case "disk": return CelestinaTheme.glyphAccentAmber
        case "network": return CelestinaTheme.glyphAccentGreen
        }
        return CelestinaTheme.glyphAccentBlue
    }
    readonly property color trace: graph.load === "critical"
                                   ? CelestinaTheme.danger
                                   : graph.load === "elevated"
                                     ? CelestinaTheme.warning
                                     : graph.kindColor

    implicitHeight: CelestinaTheme.rowHeightLg * 2

    // The plot's own size: the graph less its inset on every side.
    readonly property real plotWidth: Math.max(0, graph.width - CelestinaTheme.spaceXs * 2)
    readonly property real plotHeight: Math.max(0, graph.height - CelestinaTheme.spaceXs * 2)

    function pointX(index) {
        const count = graph.series.length
        return count <= 1 ? 0 : index * graph.plotWidth / (count - 1)
    }

    function pointY(value) {
        const clamped = Math.max(0, Math.min(1, value))
        return graph.plotHeight - clamped * graph.plotHeight
    }

    // Both paths are rebuilt from the series once per sample. A PathPolyline
    // takes the points in one assignment, which is the cheap way to redraw.
    readonly property var tracePoints: {
        const points = []
        for (let index = 0; index < graph.series.length; ++index)
            points.push(Qt.point(graph.pointX(index), graph.pointY(graph.series[index])))
        return points
    }

    readonly property var areaPoints: {
        if (graph.series.length === 0)
            return []
        const points = [Qt.point(0, graph.plotHeight)]
        for (let index = 0; index < graph.series.length; ++index)
            points.push(Qt.point(graph.pointX(index), graph.pointY(graph.series[index])))
        points.push(Qt.point(graph.plotWidth, graph.plotHeight))
        return points
    }

    Rectangle {
        anchors.fill: parent
        radius: CelestinaTheme.radiusSm
        color: CelestinaTheme.inputFill
    }

    Item {
        anchors.fill: parent
        anchors.margins: CelestinaTheme.spaceXs
        clip: true

        Shape {
            anchors.fill: parent
            preferredRendererType: Shape.CurveRenderer
            opacity: graph.series.length > 1 ? 1 : 0

            Behavior on opacity {
                NumberAnimation {
                    duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionNormal
                    easing.type: CelestinaTheme.easeStandard
                }
            }

            ShapePath {
                strokeWidth: 0
                strokeColor: CelestinaTheme.clear
                fillColor: CelestinaTheme.withAlpha(graph.trace,
                                                    CelestinaTheme.accentSoftOpacity)
                PathPolyline { path: graph.areaPoints }
            }

            ShapePath {
                strokeWidth: CelestinaTheme.borderHairline * 2
                strokeColor: graph.trace
                fillColor: CelestinaTheme.clear
                capStyle: ShapePath.RoundCap
                joinStyle: ShapePath.RoundJoin
                PathPolyline { path: graph.tracePoints }
            }
        }
    }

    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Historial del último minuto")
}
