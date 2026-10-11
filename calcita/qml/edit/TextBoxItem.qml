pragma ComponentBehavior: Bound
import QtQuick
import org.celestina.calcita 1.0

// One text box while editing, drawn by the window over the page (the copy
// QtPdf renders has it hidden): its text in Helvetica, the font the saved
// appearance names, laid out in the page's points and scaled with the page,
// so what is seen is what is written. Selected, it shows its frame and four
// handles; dragged, it moves; a handle resizes it; a double click, Enter on
// the selection or a tap with «Texto» armed edits its text, unless it
// carries rich text another program wrote. Only a box Calcita lays out (its
// own, or one whose text or colour changed: `laidOut`) is resized; another
// program's moves at its own size, keeping its own appearance. Everything it changes is reported in the displayed page's
// points; the editor decides.
Item {
    id: box

    // The annotation as the editor publishes it.
    required property var item
    required property real renderScale
    // The editor's layout: the padding around the text and the line height
    // in font sizes.
    required property real textPadding
    required property real lineHeight
    required property bool selected
    // «Seleccionar» is armed: the box moves, resizes and edits.
    required property bool interactive
    // «Texto» is armed: a tap edits the text.
    required property bool tapEdits
    // The window asks this box's text to be edited (Enter on the selection).
    required property bool editRequested

    signal selectRequested()
    // The box should fill this rectangle, in points.
    signal moveRequested(real x, real y, real width, real height)
    signal textCommitted(string text)

    property bool editingText: false
    // While dragged or resized, in points: how far the left and top edges
    // moved and how much the box grew.
    property real shiftX: 0
    property real shiftY: 0
    property real growX: 0
    property real growY: 0

    readonly property real minWidth: box.item.size + box.textPadding * 2
    readonly property bool resizable: box.item.laidOut === true
    // As the editor has it, until a handle grows it.
    readonly property real shownWidth: box.growX !== 0 ? Math.max(box.minWidth, box.item.w + box.growX)
                                                       : box.item.w
    readonly property real shownHeight: Math.max(box.item.h + box.growY,
                                                 box.editingText ? field.contentHeight + box.textPadding * 2 : 0)

    function beginEdit() {
        if (box.item.rich)
            return
        field.text = box.item.text
        box.editingText = true
        field.forceActiveFocus()
        field.cursorPosition = field.length
    }

    function commitEdit() {
        if (!box.editingText)
            return
        box.editingText = false
        if (field.text !== box.item.text)
            box.textCommitted(field.text)
    }

    // A drag or a resize ended: the box asks for where it now is.
    function finishGeometry() {
        const moved = box.shiftX !== 0 || box.shiftY !== 0 || box.growX !== 0 || box.growY !== 0
        const x = box.item.x + box.shiftX
        const y = box.item.y + box.shiftY
        const width = box.shownWidth
        const height = box.growY !== 0 ? Math.max(0, box.item.h + box.growY) : box.item.h
        box.shiftX = 0
        box.shiftY = 0
        box.growX = 0
        box.growY = 0
        if (moved)
            box.moveRequested(x, y, width, height)
    }

    onEditRequestedChanged: {
        if (box.editRequested)
            box.beginEdit()
    }

    objectName: "textBox_" + box.item.id
    x: (box.item.x + box.shiftX) * box.renderScale
    y: (box.item.y + box.shiftY) * box.renderScale
    width: box.shownWidth * box.renderScale
    height: box.shownHeight * box.renderScale
    Accessible.role: Accessible.EditableText
    Accessible.name: box.item.text
    Accessible.selectable: true
    Accessible.selected: box.selected
    Accessible.onPressAction: box.selectRequested()

    // The box in the page's points, scaled with the page.
    Item {
        id: sheet
        width: box.shownWidth
        height: box.shownHeight
        scale: box.renderScale
        transformOrigin: Item.TopLeft

        Text {
            objectName: "textBoxText"
            visible: !box.editingText
            width: sheet.width
            padding: box.textPadding
            text: box.item.text
            color: box.item.color
            // The standard font the saved appearance names; fontconfig gives
            // a metric-compatible face (Liberation or Nimbus Sans).
            font.family: "Helvetica"
            font.pixelSize: box.item.size
            lineHeightMode: Text.FixedHeight
            lineHeight: box.item.size * box.lineHeight
            wrapMode: Text.Wrap
        }

        TextEdit {
            id: field
            objectName: "textBoxField"
            visible: box.editingText
            enabled: box.editingText
            width: sheet.width
            padding: box.textPadding
            color: box.item.color
            font.family: "Helvetica"
            font.pixelSize: box.item.size
            wrapMode: TextEdit.Wrap
            selectByMouse: true
            selectionColor: CelestinaTheme.accentSoft
            selectedTextColor: box.item.color
            Accessible.name: qsTr("Texto del cuadro")
            Keys.onEscapePressed: box.commitEdit()
            onActiveFocusChanged: {
                if (!field.activeFocus)
                    box.commitEdit()
            }
        }
    }

    Rectangle {
        objectName: "textBoxFrame"
        anchors.fill: parent
        visible: box.selected || box.editingText
        color: CelestinaTheme.clear
        radius: CelestinaTheme.radiusNone
        border.width: CelestinaTheme.borderFocus
        border.color: CelestinaTheme.focusRing
    }

    DragHandler {
        id: mover
        enabled: box.interactive && !box.editingText
        target: null
        onCentroidChanged: {
            if (!mover.active)
                return
            box.shiftX = (mover.centroid.scenePosition.x - mover.centroid.scenePressPosition.x) / box.renderScale
            box.shiftY = (mover.centroid.scenePosition.y - mover.centroid.scenePressPosition.y) / box.renderScale
        }
        onActiveChanged: {
            if (mover.active)
                box.selectRequested()
            else
                box.finishGeometry()
        }
    }

    TapHandler {
        enabled: box.interactive || box.tapEdits
        gesturePolicy: TapHandler.ReleaseWithinBounds
        onTapped: {
            if (box.tapEdits)
                box.beginEdit()
            else
                box.selectRequested()
        }
        onDoubleTapped: {
            if (box.interactive)
                box.beginEdit()
        }
    }

    // The four corners; each moves its own two edges.
    Repeater {
        model: box.selected && box.interactive && !box.editingText && box.resizable ? 4 : 0

        delegate: Rectangle {
            id: handle
            required property int index
            readonly property bool atLeft: handle.index % 2 === 0
            readonly property bool atTop: handle.index < 2

            objectName: "textBoxHandle" + handle.index
            width: CelestinaTheme.spaceSm
            height: CelestinaTheme.spaceSm
            radius: CelestinaTheme.radiusPill
            color: CelestinaTheme.focusRing
            x: (handle.atLeft ? 0 : box.width) - handle.width / 2
            y: (handle.atTop ? 0 : box.height) - handle.height / 2

            DragHandler {
                id: resizer
                target: null
                onCentroidChanged: {
                    if (!resizer.active)
                        return
                    const dx = (resizer.centroid.scenePosition.x - resizer.centroid.scenePressPosition.x) / box.renderScale
                    const dy = (resizer.centroid.scenePosition.y - resizer.centroid.scenePressPosition.y) / box.renderScale
                    box.shiftX = handle.atLeft ? dx : 0
                    box.growX = handle.atLeft ? -dx : dx
                    box.shiftY = handle.atTop ? dy : 0
                    box.growY = handle.atTop ? -dy : dy
                }
                onActiveChanged: {
                    if (!resizer.active)
                        box.finishGeometry()
                }
            }
        }
    }
}
