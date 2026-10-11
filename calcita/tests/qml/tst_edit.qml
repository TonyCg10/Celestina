// language-contract: allow-non-english (the Spanish names the window shows, compared verbatim)
import QtQuick
import QtTest 1.3
import QtQuick.Pdf
import org.celestina.calcita 1.0

// Edit mode as the window carries it: Ctrl+E and «Editar» enter and leave
// it, the reading mode waits, the pages render the editor's copy, the
// bottom toolbar arms the tools, colours and sizes, and leaving or closing
// with changes asks the save question, whose three answers each do what
// they say. The editor is the QML stand-in; its Rust half is tested in
// `cargo test`.
TestCase {
    id: testCase
    name: "Edit"
    when: windowShown

    readonly property string fixture: String(Qt.resolvedUrl("../fixtures/three-pages.pdf"))
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

    function openFixture() {
        const window = createTemporaryObject(documentComponent, testCase,
                                             { "documentKey": testCase.fixture,
                                               "pdf": testPdf })
        testCase.current = window
        tryVerify(() => window.reader.loaded, 5000, "the fixture loads")
        window.requestActivate()
        tryVerify(() => window.active, 5000, "the window takes the keyboard")
        return window
    }

    function startEditing(window) {
        keyClick(Qt.Key_E, Qt.ControlModifier)
        tryVerify(() => window.editing, 2000, "edit mode began")
    }

    function test_ctrl_e_enters_and_leaves_edit_mode() {
        const window = openFixture()
        verify(!window.editing)
        verify(!window.editBar.visible, "no toolbar while reading")
        startEditing(window)
        verify(window.editBar.visible, "the toolbar while editing")
        compare(window.editBar.Accessible.name, "Herramientas de edición")
        keyClick(Qt.Key_E, Qt.ControlModifier)
        verify(!window.editing, "nothing changed, so it leaves without asking")
        verify(!window.saveQuestion.shown)
    }

    function test_the_bar_button_enters_edit_mode() {
        const window = openFixture()
        const button = findNamed(window.bar, "editButton")
        verify(button)
        compare(button.Accessible.name, "Editar")
        mouseClick(button)
        tryVerify(() => window.editing)
        compare(button.role, CelestinaButton.Selected, "it shows the mode")
    }

    function test_the_reading_mode_waits_while_editing() {
        const window = openFixture()
        window.toggleReadingMode()
        tryVerify(() => window.view.layer.enabled)
        startEditing(window)
        compare(window.readingAmount, 0, "off while editing")
        verify(!window.view.layer.enabled)
        verify(!findNamed(window.bar, "readingButton").enabled, "its button waits")
        keyClick(Qt.Key_I, Qt.ControlModifier)
        verify(window.readingDark, "Ctrl+I does nothing while editing")
        keyClick(Qt.Key_E, Qt.ControlModifier)
        tryVerify(() => window.view.layer.enabled, 2000, "back after editing")
        verify(window.readingDark, "the mode itself was kept")
    }

    function test_the_pages_render_the_editors_copy_while_editing() {
        const window = openFixture()
        const page = findNamed(window.view, "page0")
        tryVerify(() => page.item !== null, 2000)
        const image = findNamed(page.item, "pageImage")
        compare(String(image.source), "image://calcitapage/0/1/" + testCase.fixture)
        startEditing(window)
        compare(String(image.source), "image://calcitapage/0/7/" + testCase.fixture,
                "the copy's revision")
        verify(findNamed(page.item, "annotationLayer0"), "the page lays the annotation layer")
        window.editor.finish()
        compare(String(image.source), "image://calcitapage/0/1/" + testCase.fixture, "the file again")
    }

    function test_the_toolbar_arms_tools_colours_and_sizes() {
        const window = openFixture()
        startEditing(window)
        const bar = window.editBar
        compare(window.editTool, "select")
        const tools = [["tool_text", "text"], ["tool_highlight", "highlight"],
                       ["tool_ink", "ink"], ["tool_select", "select"]]
        tools.forEach(pair => {
            mouseClick(findNamed(bar, pair[0]))
            compare(window.editTool, pair[1])
        })
        compare(findNamed(bar, "tool_text").text, "Texto")
        compare(findNamed(bar, "tool_highlight").text, "Resaltar")
        compare(findNamed(bar, "tool_ink").text, "Dibujar")
        compare(findNamed(bar, "tool_select").text, "Seleccionar")

        const swatches = CelestinaTheme.iconAccentKeys
        compare(swatches.length, 6, "six theme colours")
        const coral = findNamed(bar, "swatch_coral")
        mouseClick(coral)
        compare(window.editColorKey, "coral")
        verify(coral.Accessible.checked)

        const large = findNamed(bar, "size_2")
        mouseClick(large)
        compare(window.editSizeIndex, 2)
        window.editTool = "ink"
        compare(large.Accessible.name, "Trazo grueso", "the size names the armed tool's steps")
        window.editTool = "text"
        compare(large.Accessible.name, "Texto grande")
    }

    function test_undo_redo_and_save_wait_for_something_to_do() {
        const window = openFixture()
        startEditing(window)
        const bar = window.editBar
        verify(!findNamed(bar, "undoButton").enabled)
        verify(!findNamed(bar, "redoButton").enabled)
        verify(!findNamed(bar, "saveBothButton").enabled)
        verify(!findNamed(bar, "deleteButton").enabled, "nothing selected")
        window.editor.addText(0, Qt.point(72, 72), 200, "hola", "#000000", 14)
        verify(findNamed(bar, "undoButton").enabled)
        verify(findNamed(bar, "saveBothButton").enabled)
        compare(findNamed(bar, "saveBothButton").Accessible.name, "Guardar ambas")
        compare(findNamed(bar, "saveEditedButton").Accessible.name, "Guardar solo la editada")
        mouseClick(findNamed(bar, "undoButton"))
        verify(!window.editor.edited)
        verify(findNamed(bar, "redoButton").enabled)
        mouseClick(findNamed(bar, "redoButton"))
        verify(window.editor.edited)
    }

    function test_leaving_with_changes_asks_and_keeping_both_moves_to_the_copy() {
        const window = openFixture()
        startEditing(window)
        window.editor.addText(0, Qt.point(72, 72), 200, "hola", "#000000", 14)
        keyClick(Qt.Key_E, Qt.ControlModifier)
        verify(window.saveQuestion.shown, "the question")
        verify(window.editing, "still editing until it is answered")
        const keepBoth = findNamed(window.saveQuestion, "keepBothButton")
        compare(keepBoth.text, "Guardar ambas")
        tryVerify(() => keepBoth.activeFocus, 2000, "the answer that loses nothing has the focus")
        mouseClick(keepBoth)
        compare(window.editor.saves, [false])
        verify(!window.editing)
        const copy = testCase.fixture.replace(/\.pdf$/, " (editado).pdf")
        verify(CalcitaController.remembered[copy] !== undefined,
               "the copy keeps where the original was read")
    }

    function test_saving_only_the_edited_reloads_the_document() {
        const window = openFixture()
        startEditing(window)
        window.editor.addText(0, Qt.point(72, 72), 200, "hola", "#000000", 14)
        const before = window.reader.revision
        mouseClick(findNamed(window.editBar, "saveEditedButton"))
        compare(window.editor.saves, [true])
        compare(window.reader.revision, before + 1, "a new version of the file")
        verify(!window.editing)
        tryVerify(() => window.reader.loaded, 5000, "loaded again")
    }

    function test_discarding_drops_the_changes() {
        const window = openFixture()
        startEditing(window)
        window.editor.addText(0, Qt.point(72, 72), 200, "hola", "#000000", 14)
        window.leaveEditing()
        const discard = findNamed(window.saveQuestion, "discardButton")
        compare(discard.text, "Descartar")
        mouseClick(discard)
        verify(!window.saveQuestion.shown)
        verify(!window.editing)
        compare(window.editor.saves, [], "nothing saved")
        compare(window.editor.annotations, "[]")
    }

    function test_escape_on_the_question_stays_with_the_changes() {
        const window = openFixture()
        startEditing(window)
        window.editor.addText(0, Qt.point(72, 72), 200, "hola", "#000000", 14)
        window.leaveEditing()
        tryVerify(() => findNamed(window.saveQuestion, "keepBothButton").activeFocus, 2000)
        keyClick(Qt.Key_Escape)
        verify(!window.saveQuestion.shown)
        verify(window.editing && window.editor.edited, "still editing, changes kept")
    }

    function test_closing_with_changes_asks_and_discarding_closes() {
        const window = openFixture()
        let closedKey = ""
        window.closed.connect(key => closedKey = key)
        startEditing(window)
        window.editor.addText(0, Qt.point(72, 72), 200, "hola", "#000000", 14)
        window.close()
        verify(window.visible, "the close waits for an answer")
        verify(window.saveQuestion.shown)
        mouseClick(findNamed(window.saveQuestion, "discardButton"))
        tryVerify(() => closedKey === testCase.fixture, 2000, "then it closes")
        testCase.current = null
    }

    function test_closing_after_a_save_that_lands() {
        const window = openFixture()
        let closedKey = ""
        window.closed.connect(key => closedKey = key)
        startEditing(window)
        window.editor.addText(0, Qt.point(72, 72), 200, "hola", "#000000", 14)
        window.close()
        mouseClick(findNamed(window.saveQuestion, "replaceButton"))
        compare(window.editor.saves, [true])
        tryVerify(() => closedKey !== "", 2000, "closed once saved")
        testCase.current = null
    }

    function test_a_file_changed_while_editing_loads_again_once_editing_ends() {
        const window = openFixture()
        const sources = []
        const watch = () => sources.push(String(window.pdf.source))
        window.pdf.sourceChanged.connect(watch)
        startEditing(window)
        window.reader.reload()
        compare(sources, [], "the document waits while editing")
        verify(window.reloadPending)
        window.editor.addText(0, Qt.point(72, 72), 200, "hola", "#000000", 14)
        window.leaveEditing()
        mouseClick(findNamed(window.saveQuestion, "discardButton"))
        tryVerify(() => sources.length === 2, 2000, "loaded again after «Descartar»")
        compare(sources[1], String(window.reader.documentUrl))
        verify(!window.reloadPending)
        window.pdf.sourceChanged.disconnect(watch)
        tryVerify(() => window.reader.loaded, 5000)
    }

    function test_a_file_changed_while_editing_and_then_saved_loads_once() {
        const window = openFixture()
        const sources = []
        const watch = () => sources.push(String(window.pdf.source))
        window.pdf.sourceChanged.connect(watch)
        startEditing(window)
        window.reader.reload()
        window.editor.addText(0, Qt.point(72, 72), 200, "hola", "#000000", 14)
        mouseClick(findNamed(window.editBar, "saveEditedButton"))
        wait(50)
        compare(sources.length, 2, "one load again, by the save, not two")
        verify(!window.reloadPending)
        window.pdf.sourceChanged.disconnect(watch)
        tryVerify(() => window.reader.loaded, 5000)
    }

    function test_a_save_that_does_not_land_keeps_the_window() {
        const window = openFixture()
        startEditing(window)
        window.editor.saveLands = false
        window.editor.addText(0, Qt.point(72, 72), 200, "hola", "#000000", 14)
        window.close()
        mouseClick(findNamed(window.saveQuestion, "keepBothButton"))
        compare(window.editor.saves, [false])
        wait(50)
        verify(window.visible, "nothing landed, nothing closes")
        verify(window.editing)
    }
}
