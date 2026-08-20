pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Shapes
import org.celestina.fluorita 1.0

// What has been placed on the picture, drawn over it.
//
// This is a *preview*, not the result: the bytes are produced by the toolkit
// when the edit is saved, and what is drawn here only has to show the person
// where their marks are. Two consequences are deliberate. A redaction is drawn
// as a plate rather than as a real pixelation, because approximating the
// irreversibility here would invite trusting it — the real one happens on the
// way to disk. And every geometry comes from the editor's published rows, so
// nothing on screen was computed twice.
//
// Coordinates arrive in canvas pixels and are scaled by `scaleFactor`, which is
// what keeps a mark on the same part of the photograph at any zoom and on any
// display scale.
Item {
    id: objectLayer

    required property FluoritaEditor editor
    required property real scaleFactor

    // The one byte the adapter joins a row's remaining fields with. A unit
    // separator, because it is the one character a person cannot type into a
    // text annotation and therefore cannot use to forge a second field.
    readonly property string separator: "\u001F"

    signal objectPicked(int id)

    // Rebuilt from the editor's one revision signal rather than from six lists
    // that change one at a time.
    readonly property var rows: {
        void objectLayer.editor.revision
        const built = []
        const ids = objectLayer.editor.objectIds
        for (let index = 0; index < ids.length; ++index) {
            built.push({
                id: parseInt(ids[index], 10),
                kind: objectLayer.editor.objectKinds[index],
                geometry: objectLayer.editor.objectGeometry[index],
                ink: objectLayer.editor.objectInks[index],
                width: parseFloat(objectLayer.editor.objectWidths[index]),
                detail: objectLayer.editor.objectDetails[index]
            })
        }
        return built
    }

    function numbers(text) {
        return text.split(",").map(parseFloat)
    }

    Repeater {
        model: objectLayer.rows

        delegate: Item {
            id: object

            required property var modelData

            readonly property var box: object.modelData.kind === "stroke"
                || object.modelData.kind === "line"
                ? [0, 0, 0, 0] : objectLayer.numbers(object.modelData.geometry)
            readonly property bool selected: objectLayer.editor.selected === object.modelData.id
            readonly property real scaled: objectLayer.scaleFactor

            x: object.box[0] * object.scaled
            y: object.box[1] * object.scaled
            width: object.box[2] * object.scaled
            height: object.box[3] * object.scaled
            visible: object.modelData.kind !== "stroke" && object.modelData.kind !== "line"

            Rectangle {
                anchors.fill: parent
                visible: object.modelData.kind === "shape"
                    && object.modelData.detail.split(objectLayer.separator)[0] === "rect"
                color: object.modelData.detail.split(objectLayer.separator)[1] || CelestinaTheme.clear
                border.color: object.modelData.ink
                border.width: Math.max(1, object.modelData.width * object.scaled)
                radius: CelestinaTheme.radiusNone
            }

            Rectangle {
                anchors.fill: parent
                visible: object.modelData.kind === "shape"
                    && object.modelData.detail.split(objectLayer.separator)[0] === "ellipse"
                color: object.modelData.detail.split(objectLayer.separator)[1] || CelestinaTheme.clear
                border.color: object.modelData.ink
                border.width: Math.max(1, object.modelData.width * object.scaled)
                radius: Math.min(width, height) / 2
            }

            Rectangle {
                anchors.fill: parent
                visible: object.modelData.kind === "highlight"
                color: object.modelData.ink
            }

            // The redaction's preview: opaque, so nothing under it is readable
            // on screen either, and marked so it is not mistaken for a shape.
            Rectangle {
                anchors.fill: parent
                visible: object.modelData.kind === "redact"
                color: CelestinaTheme.surfaceStrong

                CelestinaIcon {
                    anchors.centerIn: parent
                    width: Math.min(CelestinaTheme.iconSm, parent.width, parent.height)
                    height: width
                    name: "eye-off"
                    tone: CelestinaIcon.Secondary
                }
            }

            Text {
                anchors.fill: parent
                visible: object.modelData.kind === "text"
                text: object.modelData.detail.split(objectLayer.separator).slice(1).join("")
                rotation: 90 * parseInt(object.modelData.detail.split(objectLayer.separator)[0] || "0", 10)
                color: object.modelData.ink
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: Math.max(1, object.modelData.width * object.scaled)
                wrapMode: Text.Wrap
                elide: Text.ElideNone
            }

            // The selection ring is the shared focus anatomy rather than a
            // second idea of "this one".
            CelestinaFocusRing {
                target: object
                cornerRadius: CelestinaTheme.radiusXs
                visible: object.selected
            }

            TapHandler {
                onTapped: objectLayer.objectPicked(object.modelData.id)
            }
        }
    }

    // Strokes and lines have no box, so they are their own shapes.
    Repeater {
        model: objectLayer.rows.filter(row => row.kind === "stroke" || row.kind === "line")

        delegate: Shape {
            id: drawn

            required property var modelData

            anchors.fill: parent
            preferredRendererType: Shape.CurveRenderer

            readonly property var points: drawn.modelData.kind === "stroke"
                ? drawn.modelData.geometry.split(";").map(pair => objectLayer.numbers(pair))
                : [objectLayer.numbers(drawn.modelData.geometry).slice(0, 2),
                   objectLayer.numbers(drawn.modelData.geometry).slice(2, 4)]

            ShapePath {
                strokeColor: drawn.modelData.ink
                strokeWidth: Math.max(1, drawn.modelData.width * objectLayer.scaleFactor)
                capStyle: ShapePath.RoundCap
                joinStyle: ShapePath.RoundJoin
                fillColor: CelestinaTheme.clear
                startX: (drawn.points.length > 0 ? drawn.points[0][0] : 0) * objectLayer.scaleFactor
                startY: (drawn.points.length > 0 ? drawn.points[0][1] : 0) * objectLayer.scaleFactor

                PathPolyline {
                    path: drawn.points.map(point => Qt.point(point[0] * objectLayer.scaleFactor,
                                                             point[1] * objectLayer.scaleFactor))
                }
            }
        }
    }
}
