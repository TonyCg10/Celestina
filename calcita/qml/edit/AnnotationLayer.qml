pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Shapes
import org.celestina.calcita 1.0

// One page's editable annotations while editing, laid over the page the
// copy without them renders: highlights under drawings under text boxes,
// the text box being written, and the stroke being drawn. The armed tool
// decides what the pointer does on the page itself: «Texto» places a text
// box where it taps, «Dibujar» draws from press to release, «Seleccionar»
// clears the selection on a tap outside every annotation, and «Resaltar»
// leaves the page to the page view's own text selection (the window turns
// it into a highlight). Positions go to the editor in the displayed page's
// points; the editor decides.
Item {
    id: marks

    required property CalcitaEditor editor
    // `select`, `text`, `highlight` or `ink`.
    required property string tool
    required property string selectedId
    // The text box whose text the window asks to edit, or "".
    required property string editRequest
    // The colour new text and drawings take.
    required property color ink
    required property real textSize
    required property real strokeWidth
    // Set by the page that loads the layer.
    property int page: -1
    property real renderScale: 1

    signal selectRequested(string id)

    // This page's annotations, as the editor publishes them.
    readonly property var items: marks.page < 0
                                 ? [] : JSON.parse(marks.editor.annotations).filter(
                                       entry => entry.page === marks.page)
    // The top-left corner of the text box being written, in points; x is
    // -1 while there is none.
    property real draftX: -1
    property real draftY: 0
    // The stroke being drawn, in points.
    property var stroke: []

    // A new text box is 16 ems wide, inside the page.
    function draftWidth() {
        const room = marks.width / marks.renderScale - marks.draftX
        return Math.max(marks.textSize * 2, Math.min(marks.textSize * 16, room))
    }

    function placeDraft(position) {
        marks.commitDraft()
        marks.draftX = position.x / marks.renderScale
        marks.draftY = position.y / marks.renderScale
        draftField.text = ""
        draftField.forceActiveFocus()
    }

    // The text written becomes a text box; empty, the draft just goes. A
    // refused text keeps the draft so it can be fixed.
    function commitDraft() {
        if (marks.draftX < 0)
            return
        if (draftField.text.length > 0) {
            const id = marks.editor.addText(marks.page, Qt.point(marks.draftX, marks.draftY), marks.draftWidth(),
                                            draftField.text, String(marks.ink), marks.textSize)
            if (id === "")
                return
        }
        marks.draftX = -1
        draftField.text = ""
    }

    function finishStroke() {
        const points = marks.stroke.map(point => point.x + "," + point.y).join(" ")
        marks.stroke = []
        if (points.length > 0)
            marks.editor.addInk(marks.page, points, String(marks.ink), marks.strokeWidth)
    }

    objectName: "annotationLayer" + marks.page
    onToolChanged: marks.commitDraft()

    // The page itself, under every annotation.
    Item {
        anchors.fill: parent

        TapHandler {
            enabled: marks.tool === "text" || marks.tool === "select"
            gesturePolicy: TapHandler.ReleaseWithinBounds
            onTapped: eventPoint => {
                if (marks.tool === "text")
                    marks.placeDraft(eventPoint.position)
                else
                    marks.selectRequested("")
            }
        }

        DragHandler {
            id: pen
            enabled: marks.tool === "ink"
            target: null
            onActiveChanged: {
                if (pen.active)
                    marks.stroke = [Qt.point(pen.centroid.pressPosition.x / marks.renderScale,
                                             pen.centroid.pressPosition.y / marks.renderScale)]
                else
                    marks.finishStroke()
            }
            onCentroidChanged: {
                if (!pen.active)
                    return
                marks.stroke.push(Qt.point(pen.centroid.position.x / marks.renderScale,
                                           pen.centroid.position.y / marks.renderScale))
                marks.strokeChanged()
            }
        }
    }

    Repeater {
        model: marks.items.filter(entry => entry.kind === "highlight")

        delegate: HighlightItem {
            id: highlight
            required property var modelData
            item: highlight.modelData
            renderScale: marks.renderScale
            selected: marks.selectedId === highlight.modelData.id
            interactive: marks.tool === "select"
            onSelectRequested: marks.selectRequested(highlight.modelData.id)
            onMoveRequested: (x, y, width, height) => marks.editor.moveTo(highlight.modelData.id,
                                                                          x, y, width, height)
        }
    }

    Repeater {
        model: marks.items.filter(entry => entry.kind === "ink")

        delegate: InkItem {
            id: drawing
            required property var modelData
            item: drawing.modelData
            renderScale: marks.renderScale
            selected: marks.selectedId === drawing.modelData.id
            interactive: marks.tool === "select"
            onSelectRequested: marks.selectRequested(drawing.modelData.id)
            onMoveRequested: (x, y, width, height) => marks.editor.moveTo(drawing.modelData.id,
                                                                          x, y, width, height)
        }
    }

    Repeater {
        model: marks.items.filter(entry => entry.kind === "text")

        delegate: TextBoxItem {
            id: textBox
            required property var modelData
            item: textBox.modelData
            renderScale: marks.renderScale
            textPadding: marks.editor.textPadding
            lineHeight: marks.editor.lineHeight
            selected: marks.selectedId === textBox.modelData.id
            interactive: marks.tool === "select"
            tapEdits: marks.tool === "text"
            editRequested: marks.editRequest === textBox.modelData.id
            onSelectRequested: marks.selectRequested(textBox.modelData.id)
            onMoveRequested: (x, y, width, height) => marks.editor.moveTo(textBox.modelData.id,
                                                                          x, y, width, height)
            onTextCommitted: text => {
                if (text.length > 0)
                    marks.editor.setText(textBox.modelData.id, text)
                else
                    marks.editor.remove(textBox.modelData.id)
            }
        }
    }

    // The stroke being drawn.
    Shape {
        visible: marks.stroke.length > 0

        ShapePath {
            strokeColor: marks.ink
            strokeWidth: marks.strokeWidth * marks.renderScale
            fillColor: CelestinaTheme.clear
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            scale: Qt.size(marks.renderScale, marks.renderScale)
            PathPolyline {
                path: marks.stroke
            }
        }
    }

    // The text box being written.
    Item {
        id: draft
        objectName: "draftTextBox"
        visible: marks.draftX >= 0
        x: marks.draftX * marks.renderScale
        y: marks.draftY * marks.renderScale
        width: marks.draftWidth() * marks.renderScale
        height: (draftField.contentHeight + marks.editor.textPadding * 2) * marks.renderScale

        Item {
            width: marks.draftWidth()
            height: draft.height / marks.renderScale
            scale: marks.renderScale
            transformOrigin: Item.TopLeft

            TextEdit {
                id: draftField
                objectName: "draftField"
                enabled: marks.draftX >= 0
                width: parent.width
                padding: marks.editor.textPadding
                color: marks.ink
                // The standard font the saved appearance names.
                font.family: "Helvetica"
                font.pixelSize: marks.textSize
                wrapMode: TextEdit.Wrap
                selectByMouse: true
                selectionColor: CelestinaTheme.accentSoft
                selectedTextColor: marks.ink
                Accessible.name: qsTr("Texto nuevo")
                Keys.onEscapePressed: marks.commitDraft()
                onActiveFocusChanged: {
                    if (!draftField.activeFocus)
                        marks.commitDraft()
                }
            }
        }

        Rectangle {
            anchors.fill: parent
            color: CelestinaTheme.clear
            radius: CelestinaTheme.radiusNone
            border.width: CelestinaTheme.borderFocus
            border.color: CelestinaTheme.focusRing
        }
    }
}
