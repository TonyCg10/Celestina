import QtQuick
import QtTest 1.3
import QtQuick.Pdf
import org.celestina.calcita 1.0

// The tools on a page: «Texto» places a box where it taps and adds what is
// typed; «Seleccionar» selects, moves and resizes by the handles, and
// double-click edits; Supr deletes and Ctrl+Z / Ctrl+Shift+Z undo and redo;
// «Dibujar» adds the stroke from press to release; «Resaltar» turns QtPdf's
// text selection into a highlight. Positions reach the editor in the
// displayed page's points. The editor is the QML stand-in.
TestCase {
    id: testCase
    name: "EditTools"
    when: windowShown

    readonly property string fixture: String(Qt.resolvedUrl("../fixtures/outline.pdf"))
                                      .substring("file://".length)

    PdfDocument {
        id: testPdf
    }

    Component {
        id: documentComponent
        DocumentWindow {}
    }

    function findNamed(node, name) {
        if (node.objectName === name)
            return node
        const kids = node.children !== undefined ? node.children : []
        let found = null
        let i = 0
        while (found === null && i < kids.length) {
            found = findNamed(kids[i], name)
            i += 1
        }
        return found
    }

    property var current: null

    function init() {
        CalcitaController.reset()
        CelestinaTheme.reducedMotion = true
    }

    function cleanup() {
        if (testCase.current !== null) {
            testCase.current.editor.finish()
            testCase.current.close()
        }
        testCase.current = null
        CelestinaTheme.reducedMotion = false
        wait(0)
    }

    // The window editing the outline fixture (its first page reads «Page
    // one» at 72, 760), with `seed` already in the document; its first
    // page's annotation layer.
    function editFixture(seed) {
        const window = createTemporaryObject(documentComponent, testCase,
                                             { "documentKey": testCase.fixture,
                                               "pdf": testPdf })
        testCase.current = window
        tryVerify(() => window.reader.loaded, 5000, "the fixture loads")
        window.requestActivate()
        tryVerify(() => window.active, 5000)
        window.editor.seed = seed
        keyClick(Qt.Key_E, Qt.ControlModifier)
        tryVerify(() => window.editing, 2000)
        const page = findNamed(window.view, "page0")
        tryVerify(() => page.item !== null, 2000)
        let layer = null
        tryVerify(() => (layer = findNamed(page.item, "annotationLayer0")) !== null, 2000)
        return layer
    }

    // A drag of (`dx`, `dy`) from (`x`, `y`) in `item`, in steps.
    function drag(item, x, y, dx, dy) {
        mousePress(item, x, y)
        for (const step of [1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
            mouseMove(item, x + dx * step / 10, y + dy * step / 10)
        mouseRelease(item, x + dx, y + dy)
    }

    // A box Calcita lays out (its text changed this session, say).
    readonly property var textSeed: [{ "id": "o6.0", "kind": "text", "page": 0, "x": 100, "y": 200,
                                       "w": 200, "h": 30, "color": "#000099", "text": "Hola",
                                       "size": 14, "rich": false, "laidOut": true }]

    function test_the_text_tool_places_a_box_and_adds_what_is_typed() {
        const layer = editFixture([])
        const window = testCase.current
        window.editTool = "text"
        const scale = layer.renderScale
        mouseClick(layer, 100 * scale, 300 * scale)
        const draft = findNamed(layer, "draftTextBox")
        verify(draft.visible, "a box where it tapped")
        tryVerify(() => findNamed(layer, "draftField").activeFocus, 2000, "ready to type")
        verify(window.editTyping, "the edit keys are the field's")
        keyClick("H")
        keyClick("o")
        keyClick("l")
        keyClick("a")
        keyClick(Qt.Key_Escape)
        verify(!draft.visible)
        const items = JSON.parse(window.editor.annotations)
        compare(items.length, 1)
        compare(items[0].text, "Hola")
        fuzzyCompare(items[0].x, 100, 0.5)
        fuzzyCompare(items[0].y, 300, 0.5)
        compare(items[0].size, window.textSizes[window.editSizeIndex])
        compare(items[0].color, String(window.inkFor(window.editColorKey)),
                "a deep ink of the accent")
        tryVerify(() => findNamed(layer, "textBox_" + items[0].id) !== null, 2000, "drawn on the page")
    }

    function test_an_empty_box_adds_nothing() {
        const layer = editFixture([])
        const window = testCase.current
        window.editTool = "text"
        mouseClick(layer, 100 * layer.renderScale, 300 * layer.renderScale)
        keyClick(Qt.Key_Escape)
        compare(window.editor.annotations, "[]")
        verify(!window.editor.calls.some(call => call === "addText"))
    }

    function test_select_moves_a_box_and_its_handles_resize_it() {
        const layer = editFixture(testCase.textSeed)
        const window = testCase.current
        const scale = layer.renderScale
        let box = null
        tryVerify(() => (box = findNamed(layer, "textBox_o6.0")) !== null, 2000)
        mouseClick(box, box.width / 2, box.height / 2)
        compare(window.editSelected, "o6.0")
        verify(findNamed(box, "textBoxFrame").visible, "selected, framed")
        for (const corner of [0, 1, 2, 3])
            verify(findNamed(box, "textBoxHandle" + corner), "handle " + corner)
        verify(findNamed(window.editBar, "deleteButton").enabled)

        // Driven in the layer's coordinates, which do not move with the box.
        drag(layer, 200 * scale, 215 * scale, 50 * scale, 20 * scale)
        tryVerify(() => window.editor.calls.some(call => call.startsWith("moveTo:o6.0:150,220,200")),
                  2000, "moved by the drag, in points")

        tryVerify(() => (box = findNamed(layer, "textBox_o6.0")) !== null, 2000)
        mouseClick(box, box.width / 2, box.height / 2)
        verify(findNamed(box, "textBoxHandle3"), "the bottom-right handle")
        // The bottom-right corner of the box at (150, 220), 200 × 30 points.
        drag(layer, 350 * scale, 250 * scale, 40 * scale, 0)
        tryVerify(() => window.editor.calls.some(call => call.startsWith("moveTo:o6.0:150,220,240")),
                  2000, "wider by the bottom-right handle")
    }

    // A double click is the pointer's way in; qmltestrunner spaces its
    // synthetic clicks past the double-click interval, so the keyboard's
    // way in (Enter on the selection) is what is driven here.
    function test_another_programs_box_moves_at_its_own_size_without_handles() {
        const seed = [Object.assign({}, testCase.textSeed[0], { "laidOut": false, "w": 12, "h": 9 })]
        const layer = editFixture(seed)
        const window = testCase.current
        const scale = layer.renderScale
        let box = null
        tryVerify(() => (box = findNamed(layer, "textBox_o6.0")) !== null, 2000)
        fuzzyCompare(box.width, 12 * scale, 0.01, "shown at its own width, under the minimum")
        mouseClick(box, box.width / 2, box.height / 2)
        compare(window.editSelected, "o6.0")
        verify(findNamed(box, "textBoxFrame").visible)
        verify(!findNamed(box, "textBoxHandle0"), "no handles: its size is its program's")
        drag(layer, 106 * scale, 204 * scale, 30 * scale, 10 * scale)
        tryVerify(() => window.editor.calls.some(call => call === "moveTo:o6.0:130,210,12"),
                  2000, "moved, same width")
        compare(JSON.parse(window.editor.annotations)[0].h, 9, "same height")
    }

    function test_enter_on_the_selection_edits_the_text() {
        const layer = editFixture(testCase.textSeed)
        const window = testCase.current
        let box = null
        tryVerify(() => (box = findNamed(layer, "textBox_o6.0")) !== null, 2000)
        mouseClick(box, box.width / 2, box.height / 2)
        compare(window.editSelected, "o6.0")
        keyClick(Qt.Key_Return)
        const field = findNamed(box, "textBoxField")
        tryVerify(() => field.activeFocus, 2000, "the text is being edited")
        field.selectAll()
        keyClick("Y")
        keyClick(Qt.Key_Escape)
        verify(window.editor.calls.some(call => call === "setText:o6.0:Y"))
    }

    function test_rich_text_is_not_rewritten() {
        const rich = [Object.assign({}, testCase.textSeed[0], { "rich": true })]
        const layer = editFixture(rich)
        let box = null
        tryVerify(() => (box = findNamed(layer, "textBox_o6.0")) !== null, 2000)
        mouseClick(box, box.width / 2, box.height / 2)
        keyClick(Qt.Key_Return)
        verify(!findNamed(box, "textBoxField").activeFocus, "no editing")
    }

    function test_delete_then_undo_and_redo() {
        const layer = editFixture(testCase.textSeed)
        const window = testCase.current
        let box = null
        tryVerify(() => (box = findNamed(layer, "textBox_o6.0")) !== null, 2000)
        mouseClick(box, box.width / 2, box.height / 2)
        compare(window.editSelected, "o6.0")
        keyClick(Qt.Key_Delete)
        compare(window.editor.annotations, "[]", "Supr deleted it")
        compare(window.editSelected, "")
        keyClick(Qt.Key_Z, Qt.ControlModifier)
        compare(JSON.parse(window.editor.annotations).length, 1, "Ctrl+Z brought it back")
        keyClick(Qt.Key_Z, Qt.ControlModifier | Qt.ShiftModifier)
        compare(window.editor.annotations, "[]", "Ctrl+Shift+Z deleted it again")
    }

    function test_a_colour_picked_recolours_the_selection() {
        const layer = editFixture(testCase.textSeed)
        const window = testCase.current
        let box = null
        tryVerify(() => (box = findNamed(layer, "textBox_o6.0")) !== null, 2000)
        mouseClick(box, box.width / 2, box.height / 2)
        mouseClick(findNamed(window.editBar, "swatch_green"))
        verify(window.editor.calls.some(call => call === "recolor:o6.0"))
        compare(JSON.parse(window.editor.annotations)[0].color,
                String(window.inkFor("green")))
    }

    function test_the_ink_tool_draws_from_press_to_release() {
        const layer = editFixture([])
        const window = testCase.current
        window.editTool = "ink"
        const scale = layer.renderScale
        mousePress(layer, 100 * scale, 400 * scale)
        for (const step of [1, 2, 3, 4, 5, 6])
            mouseMove(layer, (100 + step * 10) * scale, (400 + step * 5) * scale)
        mouseRelease(layer, 160 * scale, 430 * scale)
        tryVerify(() => window.editor.calls.some(call => call === "addInk"), 2000)
        const items = JSON.parse(window.editor.annotations)
        compare(items.length, 1)
        compare(items[0].kind, "ink")
        compare(items[0].size, window.strokeWidths[window.editSizeIndex])
        verify(items[0].shape[0].length >= 4, "a stroke of points")
        tryVerify(() => findNamed(layer, "ink_" + items[0].id) !== null, 2000)
    }

    function test_the_highlight_tool_turns_selected_text_into_a_highlight() {
        const layer = editFixture([])
        const window = testCase.current
        window.editTool = "highlight"
        const scale = layer.renderScale
        // «Page one», 24 pt at (72, 760): its line runs about 62 to 88 down
        // the page as shown.
        const view = window.view
        const page = findNamed(view, "page0").item
        mousePress(page, 74 * scale, 76 * scale)
        for (const step of [1, 2, 3, 4, 5])
            mouseMove(page, (74 + step * 20) * scale, 76 * scale)
        mouseRelease(page, 174 * scale, 76 * scale)
        tryVerify(() => window.editor.calls.some(call => call === "addHighlight:1"), 2000,
                  "QtPdf's one outline of the selected line went to the editor")
        const items = JSON.parse(window.editor.annotations)
        compare(items[0].kind, "highlight")
        compare(items[0].color, String(window.tintFor(window.editColorKey)), "the accent itself")
        compare(view.selectedText, "", "the selection became the highlight")
    }

    function test_links_wait_while_editing() {
        const layer = editFixture([])
        const window = testCase.current
        const link = findNamed(window.view, "link0_0")
        verify(link, "the outline fixture's first link")
        const before = window.view.contentY
        window.editTool = "select"
        mouseClick(link, link.width / 2, link.height / 2)
        compare(window.view.contentY, before, "the link did not move the view")
    }
}
