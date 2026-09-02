pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Shapes
import org.celestina.fluorita 1.0

// Editing one picture, inside the viewer it was opened in.
//
// The surface holds three things and confuses none of them: the picture as the
// toolkit reads it, the objects the editor has accepted, and the one shape
// currently being dragged, which belongs to nobody until the pointer is
// released. Only then does it become a request, and only if the editor accepts
// it does it appear in the layer above.
//
// **Every coordinate converts once.** The picture is fitted into the surface,
// and `scaleFactor` is the ratio between the canvas the editor computes and the
// rectangle it is drawn in. Pointer positions are divided by it on the way in
// and multiplied by it on the way out, so a mark is stored where it was drawn
// on the photograph rather than where it was drawn on this display.
Item {
    id: surface

    required property FluoritaEditor editor

    // The armed tool. `none` means the pointer selects and moves rather than
    // drawing.
    property string tool: "none"

    // The colour new objects are drawn in, chosen in the toolbar. Tokens all
    // the way down: the palette is the style's, the domain only carries the
    // bytes it was handed.
    property color inkColour: CelestinaTheme.danger

    readonly property string ink: surface.hex(surface.inkColour)
    // The same colour as a wash. A highlighter is the ink you picked, thinned,
    // rather than a second colour to choose.
    readonly property string highlightInk: surface.hex(surface.inkColour, 0.35)
    readonly property string backdropInk: surface.hex(CelestinaTheme.scrim, 0.65)

    // How thick a new stroke is, in canvas pixels, so it is the same weight on
    // a small screenshot and a large photograph.
    readonly property real strokeWidth: Math.max(2, surface.editor.canvasWidth / 320)
    readonly property real textSize: Math.max(12, surface.editor.canvasWidth / 24)

    signal closed()

    function hex(colour, alphaOverride) {
        const part = value => Math.round(Math.max(0, Math.min(1, value)) * 255)
            .toString(16).padStart(2, "0")
        const alpha = alphaOverride === undefined ? colour.a : alphaOverride
        return "#" + part(colour.r) + part(colour.g) + part(colour.b) + part(alpha)
    }

    // The room left for the picture: everything the toolbar and the caption
    // under it do not occupy. Measured rather than guessed at with a constant,
    // which is what left a 16:9 photograph pinned to the top of the window with
    // half the surface empty below it.
    readonly property real roomWidth: Math.max(1, surface.width - CelestinaTheme.spaceLg * 2)
    readonly property real roomHeight: Math.max(
        1,
        surface.height - toolbar.height - caption.height
            - CelestinaTheme.spaceLg * 2 - CelestinaTheme.spaceMd * 2)

    // The picture's drawn rectangle, fitted inside that room. Everything else
    // on this surface is positioned against it.
    readonly property real fitted: Math.min(
        surface.roomWidth / Math.max(1, surface.editor.canvasWidth),
        surface.roomHeight / Math.max(1, surface.editor.canvasHeight))
    // What one image pixel measures on screen: the fit, times how close the
    // person has moved. Every conversion between a pointer and the picture goes
    // through this one number — a mark drawn while zoomed in must land where it
    // was drawn, not where the unzoomed fit would have put it.
    readonly property real scaleFactor: surface.fitted * zoomer.level

    readonly property real drawnWidth: Math.max(1, surface.editor.canvasWidth * surface.scaleFactor)
    readonly property real drawnHeight: Math.max(1, surface.editor.canvasHeight * surface.scaleFactor)

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Edición de imagen")

    CelestinaBackdrop {
        anchors.fill: parent
    }

    // The picture. The same toolkit path the viewer uses, with the same
    // orientation handling, so what is edited is what was being looked at.
    //
    // The preview is composed from what the document says rather than from the
    // transformation list: the visible part of the file, then the turn and the
    // mirror around the centre. A surface that recomputed that mapping would be
    // the second implementation of it, and the one on screen is the one that
    // would silently disagree with the file that gets written.
    Item {
        id: canvas

        // Centred in its room, not hung from the top edge, and offset by
        // however far the picture has been dragged while zoomed in.
        x: (surface.width - surface.drawnWidth) / 2 + zoomer.panX
        y: Math.max(CelestinaTheme.spaceMd,
                    (surface.roomHeight - surface.drawnHeight) / 2 + CelestinaTheme.spaceMd)
            + zoomer.panY
        width: surface.drawnWidth
        height: surface.drawnHeight
        clip: true

        readonly property var visibleArea: {
            const parts = surface.editor.previewSource.split(",").map(parseFloat)
            return parts.length === 4 && parts.every(value => !isNaN(value))
                ? parts
                : [0, 0, Math.max(1, surface.editor.baseWidth),
                   Math.max(1, surface.editor.baseHeight)]
        }
        // The rectangle before the turn: a quarter turn swaps the two sides.
        readonly property bool turned: surface.editor.previewQuarters % 2 === 1
        readonly property real rotorWidth: canvas.turned ? canvas.height : canvas.width
        readonly property real rotorHeight: canvas.turned ? canvas.width : canvas.height
        // From the file's own pixels to the rectangle it is drawn in.
        readonly property real magnification: canvas.rotorWidth
            / Math.max(1, canvas.visibleArea[2])

        Item {
            id: rotor

            anchors.centerIn: parent
            width: canvas.rotorWidth
            height: canvas.rotorHeight
            clip: true
            rotation: 90 * surface.editor.previewQuarters
            // The mirror is applied before the turn, which is the order the
            // orientation itself is defined in.
            transform: Scale {
                origin.x: rotor.width / 2
                xScale: surface.editor.previewMirrored ? -1 : 1
            }

            Image {
                source: surface.editor.sourceUrl
                asynchronous: true
                autoTransform: true
                fillMode: Image.Stretch
                cache: false
                width: surface.editor.baseWidth * canvas.magnification
                height: surface.editor.baseHeight * canvas.magnification
                x: -canvas.visibleArea[0] * canvas.magnification
                y: -canvas.visibleArea[1] * canvas.magnification
                // Decoded at the size it is drawn at rather than at the
                // photograph's own, which on a 4000-pixel file is the
                // difference between a preview and an allocation.
                sourceSize.width: Math.ceil(width * Screen.devicePixelRatio)
                sourceSize.height: Math.ceil(height * Screen.devicePixelRatio)
            }
        }

        EditObjectLayer {
            id: objects

            anchors.fill: parent
            editor: surface.editor
            scaleFactor: surface.scaleFactor
            onObjectPicked: function(id) { surface.editor.selectObject(id) }
        }

        // The shape being dragged right now. It belongs to no one: it is not in
        // the document, it cannot be undone, and releasing the pointer is what
        // turns it into a request.
        Shape {
            anchors.fill: parent
            visible: drag.active && surface.tool !== "none"
            preferredRendererType: Shape.CurveRenderer

            ShapePath {
                strokeColor: surface.tool === "highlight"
                    ? surface.highlightInk : surface.ink
                strokeWidth: Math.max(1, surface.strokeWidth * surface.scaleFactor)
                fillColor: CelestinaTheme.clear
                capStyle: ShapePath.RoundCap
                startX: drag.startX
                startY: drag.startY

                PathPolyline {
                    path: drag.freehand
                        ? drag.trail
                        : [Qt.point(drag.startX, drag.startY), Qt.point(drag.currentX, drag.currentY)]
                }
            }
        }

        // The box being dragged, for every tool whose result is a box.
        Rectangle {
            visible: drag.active && drag.boxed
            x: Math.min(drag.startX, drag.currentX)
            y: Math.min(drag.startY, drag.currentY)
            width: Math.abs(drag.currentX - drag.startX)
            height: Math.abs(drag.currentY - drag.startY)
            color: CelestinaTheme.clear
            border.color: surface.ink
            border.width: CelestinaTheme.borderHairline
        }

        // One handler for every tool: which shape it produces is the tool's
        // business, and the conversion into canvas pixels happens in one place.
        MouseArea {
            id: drag

            anchors.fill: parent
            enabled: !surface.editor.saving
            acceptedButtons: Qt.LeftButton
            cursorShape: surface.tool === "none" ? Qt.ArrowCursor : Qt.CrossCursor

            property bool active: false
            property real startX: 0
            property real startY: 0
            property real currentX: 0
            property real currentY: 0
            property var trail: []

            readonly property bool freehand: surface.tool === "stroke"
            readonly property bool boxed: surface.tool === "crop"
                || surface.tool === "rect" || surface.tool === "ellipse"
                || surface.tool === "highlight" || surface.tool === "redact"
                || surface.tool === "text"

            function toCanvas(value) {
                return value / Math.max(0.000001, surface.scaleFactor)
            }

            onPressed: function(event) {
                if (surface.tool === "none") {
                    // Selecting, not drawing. The press is handed on rather
                    // than swallowed: this area sits over the object layer,
                    // and the `TapHandler` on each object beneath it is the
                    // only path to a non-zero selection. Clearing first means
                    // a click on empty canvas ends with nothing selected, and
                    // a click on an object ends with that one — the tap
                    // arrives after this and overrides the clear.
                    surface.editor.selectObject(0)
                    event.accepted = false
                    return
                }
                drag.active = true
                drag.startX = event.x
                drag.startY = event.y
                drag.currentX = event.x
                drag.currentY = event.y
                drag.trail = [Qt.point(event.x, event.y)]
            }

            onPositionChanged: function(event) {
                if (!drag.active) {
                    return
                }
                drag.currentX = event.x
                drag.currentY = event.y
                if (drag.freehand) {
                    // Sampled rather than every pixel: a stroke is bounded, and
                    // a point every few pixels is more than a finger can see.
                    const last = drag.trail[drag.trail.length - 1]
                    if (Math.abs(last.x - event.x) + Math.abs(last.y - event.y) > 2) {
                        drag.trail = drag.trail.concat([Qt.point(event.x, event.y)])
                    }
                }
            }

            onReleased: {
                if (!drag.active) {
                    return
                }
                drag.active = false
                surface.commit()
            }

            onCanceled: {
                drag.active = false
                drag.trail = []
            }
        }
    }

    // Turns whatever was just dragged into one request.
    //
    // Every geometry crosses as `x,y,…` in canvas pixels, which is the spelling
    // the editor publishes its objects back in. One shape in both directions
    // rather than two.
    function commit() {
        const left = Math.min(drag.startX, drag.currentX) / surface.scaleFactor
        const top = Math.min(drag.startY, drag.currentY) / surface.scaleFactor
        const width = Math.abs(drag.currentX - drag.startX) / surface.scaleFactor
        const height = Math.abs(drag.currentY - drag.startY) / surface.scaleFactor
        const area = [left, top, width, height].join(",")

        switch (surface.tool) {
        case "crop":
            surface.editor.crop(area)
            surface.tool = "none"
            break
        case "rect":
            surface.editor.addShape(false, area, surface.strokeWidth, surface.ink, "")
            break
        case "ellipse":
            surface.editor.addShape(true, area, surface.strokeWidth, surface.ink, "")
            break
        case "highlight":
            surface.editor.addHighlight(area, surface.highlightInk)
            break
        case "redact":
            surface.editor.addRedaction(area, false)
            break
        case "text":
            textPrompt.ask(area)
            break
        case "line":
        case "arrow":
            surface.editor.addLine([drag.startX / surface.scaleFactor,
                                    drag.startY / surface.scaleFactor,
                                    drag.currentX / surface.scaleFactor,
                                    drag.currentY / surface.scaleFactor].join(","),
                                   surface.strokeWidth, surface.ink,
                                   surface.tool === "arrow")
            break
        case "stroke":
            surface.editor.addStroke(
                drag.trail
                    .map(point => (point.x / surface.scaleFactor) + "," + (point.y / surface.scaleFactor))
                    .join(";"),
                surface.strokeWidth, surface.ink)
            break
        default:
            break
        }
        drag.trail = []
    }

    ZoomController {
        id: zoomer

        viewportWidth: surface.roomWidth
        viewportHeight: surface.roomHeight
        contentWidth: surface.editor.canvasWidth * surface.fitted
        contentHeight: surface.editor.canvasHeight * surface.fitted
    }

    // Ctrl and the wheel, at the pointer, exactly as in the viewer.
    WheelHandler {
        acceptedModifiers: Qt.ControlModifier
        onWheel: function(event) {
            zoomer.byWheel(event.angleDelta.y,
                           event.x - (surface.width - surface.roomWidth) / 2,
                           event.y)
        }
    }

    // Panning is the middle button, always. The left one belongs to whatever
    // tool is armed, and a picture that panned under a half-drawn stroke would
    // be the drawing tool fighting the view.
    DragHandler {
        enabled: zoomer.zoomed
        acceptedButtons: Qt.MiddleButton
        cursorShape: Qt.ClosedHandCursor
        target: null

        property real lastX: 0
        property real lastY: 0

        onActiveChanged: {
            lastX = centroid.position.x
            lastY = centroid.position.y
        }
        onCentroidChanged: {
            if (!active) {
                return
            }
            zoomer.nudge(centroid.position.x - lastX, centroid.position.y - lastY)
            lastX = centroid.position.x
            lastY = centroid.position.y
        }
    }

    // A picture that has just been opened, turned or cropped is looked at
    // whole: the old offset would be pointing into a canvas that no longer has
    // that part where it was.
    Connections {
        target: surface.editor
        function onCanvasWidthChanged() { zoomer.reset() }
        function onCanvasHeightChanged() { zoomer.reset() }
    }

    // What the picture will become, said before it is saved rather than after.
    Row {
        id: caption

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: toolbar.top
        anchors.bottomMargin: CelestinaTheme.spaceSm
        spacing: CelestinaTheme.spaceMd

        CelestinaSectionLabel {
            text: surface.editor.canvasWidth + " × " + surface.editor.canvasHeight
        }

        CelestinaSectionLabel {
            text: surface.editor.classLabel
        }

        CelestinaSectionLabel {
            visible: surface.editor.containerNotice.length > 0
            text: surface.editor.containerNotice
        }
    }

    EditToolbar {
        id: toolbar

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceLg
        editor: surface.editor
        tool: surface.tool
        ink: surface.inkColour
        zoomed: zoomer.zoomed

        onZoomToggled: zoomer.toggle()
        onToolPicked: function(picked) { surface.tool = picked }
        onInkPicked: function(colour) { surface.inkColour = colour }
        onTurned: function(clockwise) { surface.editor.rotate(clockwise) }
        onMirrored: function(horizontal) { surface.editor.flip(horizontal) }
        onSaveRequested: function(replace) { surface.editor.save(replace) }
        onDiscardRequested: surface.closed()
    }

    // What happened, in words, wherever it happened.
    CelestinaSectionLabel {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: caption.top
        anchors.bottomMargin: CelestinaTheme.spaceSm
        visible: surface.editor.notice.length > 0
        text: surface.editor.notice
    }

    // Writing is not instant on a large photograph, and a surface that looks
    // idle while it happens invites a second click.
    //
    // Raised above the toolbar and the canvas: the shield stacks itself at
    // `z: -1` inside its parent, so as a bare sibling it only covered the
    // empty backdrop while the magnifier, the swatches and the pan kept
    // acting through a save. The wheel, which the shield leaves alone by
    // contract, is held here too — a zoom during a write is not content
    // scrolling under chrome.
    Item {
        anchors.fill: parent
        z: 1

        CelestinaInputShield {
            active: surface.editor.saving
        }

        WheelHandler {
            enabled: surface.editor.saving
            target: null
            onWheel: function(event) { event.accepted = true }
        }
    }

    // The words for a text annotation, asked for before they are placed.
    //
    // The suite's modal and field rather than a Qt `Dialog`: focus containment,
    // Escape and the dismissing click are already solved once, and a second
    // dialog anatomy in one desktop is what the shared style exists to prevent.
    CelestinaModalLayer {
        id: textPrompt

        // Where the words will land, as `x,y,width,height` in canvas pixels.
        property string area: ""

        anchors.fill: parent
        onDismissRequested: textPrompt.cancel()

        function ask(area) {
            textPrompt.area = area
            words.text = ""
            textPrompt.shown = true
            words.forceActiveFocus()
        }

        function cancel() {
            textPrompt.shown = false
            surface.tool = "none"
        }

        function confirm() {
            if (words.text.length > 0) {
                surface.editor.addText(textPrompt.area, surface.textSize, surface.ink,
                                       surface.backdropInk, words.text)
            }
            textPrompt.cancel()
        }

        GlassCard {
            anchors.centerIn: parent
            width: Math.min(420, surface.width - CelestinaTheme.spaceXl * 2)
            height: heading.implicitHeight + words.implicitHeight
                + confirm.implicitHeight + CelestinaTheme.spaceLg * 4

            // A click inside the card is not a click outside the modal.
            MouseArea {
                anchors.fill: parent
            }

            Column {
                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceLg
                spacing: CelestinaTheme.spaceMd

                CelestinaSectionLabel {
                    id: heading

                    text: qsTr("Texto")
                }

                CelestinaTextField {
                    id: words

                    width: parent.width
                    placeholderText: qsTr("Escribe aquí")
                    onAccepted: textPrompt.confirm()
                }

                CelestinaButton {
                    id: confirm

                    anchors.right: parent.right
                    text: qsTr("Colocar")
                    role: CelestinaButton.Primary
                    onClicked: textPrompt.confirm()
                }
            }
        }
    }

    // The keyboard reaches everything the pointer does. Delete acts on the
    // selection, Escape leaves the tool before it leaves the editor, and the
    // two save outcomes are the two the toolbar offers.
    Keys.onPressed: function(event) {
        if (surface.editor.saving) {
            return
        }
        if (event.key === Qt.Key_Escape) {
            if (surface.tool !== "none") {
                surface.tool = "none"
            } else if (surface.editor.selected !== 0) {
                surface.editor.selectObject(0)
            } else {
                surface.closed()
            }
            event.accepted = true
        } else if (event.key === Qt.Key_Delete || event.key === Qt.Key_Backspace) {
            if (surface.editor.selected !== 0) {
                surface.editor.removeObject(surface.editor.selected)
                event.accepted = true
            }
        } else if (event.matches(StandardKey.Undo)) {
            surface.editor.undo()
            event.accepted = true
        } else if (event.matches(StandardKey.Redo)) {
            surface.editor.redo()
            event.accepted = true
        } else if (event.matches(StandardKey.Save)) {
            surface.editor.save(false)
            event.accepted = true
        } else if (surface.editor.selected !== 0
                   && (event.key === Qt.Key_Left || event.key === Qt.Key_Right
                       || event.key === Qt.Key_Up || event.key === Qt.Key_Down)) {
            // A nudge in canvas pixels, so the keyboard moves a mark by the
            // same amount whatever the picture is being shown at.
            const step = event.modifiers & Qt.ShiftModifier ? 10 : 1
            surface.editor.moveObject(
                surface.editor.selected,
                (event.key === Qt.Key_Left ? -step : event.key === Qt.Key_Right ? step : 0),
                (event.key === Qt.Key_Up ? -step : event.key === Qt.Key_Down ? step : 0))
            event.accepted = true
        }
    }
}
