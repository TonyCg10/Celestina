// language-contract: allow-non-english (the Spanish notices the window shows, compared verbatim)
import QtQuick
import QtTest 1.3
import QtQuick.Pdf
import org.celestina.calcita 1.0

// A form as the window carries it: each page lays its fields over itself; a
// tap on a field while reading starts editing on it; while editing, text
// fields and checkboxes are controls whose values go to the editor, Tab and
// Shift+Tab follow the form's order (to a widget on the next page too), a
// field Calcita only shows says why, and the values join the edit set, its
// undo and the save question. The fields are those of `form.pdf`, in its
// displayed points, published by the QML stand-ins; their Rust half is
// tested in `cargo test`.
TestCase {
    id: testCase
    name: "Form"
    when: windowShown

    readonly property string fixture: String(Qt.resolvedUrl("../fixtures/form.pdf"))
                                      .substring("file://".length)
    readonly property string radioNotice: "Calcita aún no rellena botones de opción: se muestran como están."

    function widget(order, extra) {
        return Object.assign({ "order": order, "widget": 0, "page": 0, "value": "", "fillable": true,
                               "notice": "", "checked": false, "multiline": false, "maxLen": 0,
                               "size": 12, "align": "left", "color": "#000000" }, extra)
    }

    readonly property var fields: [
        widget(0, { "field": "o8.0", "name": "name", "label": "Full name", "kind": "text", "x": 72, "y": 162, "w": 300, "h": 30 }),
        widget(1, { "field": "o11.0", "name": "agree", "label": "I agree", "kind": "checkbox", "x": 72, "y": 232, "w": 20, "h": 20 }),
        widget(2, { "field": "o15.0", "name": "address.city", "label": "city", "kind": "text", "x": 72, "y": 292, "w": 300,
                    "h": 30, "align": "center", "color": "#000099" }),
        widget(3, { "field": "o17.0", "name": "address.zip", "label": "zip", "kind": "text", "x": 72, "y": 342, "w": 100,
                    "h": 30, "maxLen": 5 }),
        widget(4, { "field": "o19.0", "name": "copy", "label": "copy", "kind": "text", "x": 400, "y": 152, "w": 150, "h": 50,
                    "multiline": true, "value": "Twice", "size": 10 }),
        widget(5, { "field": "o19.0", "name": "copy", "label": "copy", "kind": "text", "page": 1, "widget": 1, "x": 72,
                    "y": 92, "w": 150, "h": 50, "multiline": true, "value": "Twice", "size": 10 }),
        widget(6, { "field": "o24.0", "name": "choice", "label": "choice", "kind": "other", "x": 72, "y": 422, "w": 20,
                    "h": 20, "fillable": false, "notice": testCase.radioNotice })
    ]

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

    // The field (its `order`) whose control has the keyboard, or -1.
    function focusedField(window) {
        let item = window.activeFocusItem
        while (item !== null && !String(item.objectName).startsWith("formField_"))
            item = item.parent
        return item === null ? -1 : Number(String(item.objectName).substring("formField_".length))
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

    function openForm() {
        const window = createTemporaryObject(documentComponent, testCase,
                                             { "documentKey": testCase.fixture,
                                               "pdf": testPdf })
        testCase.current = window
        window.editor.seedFields = testCase.fields
        window.reader.fields = JSON.stringify(testCase.fields)
        tryVerify(() => window.reader.loaded, 5000, "the fixture loads")
        window.requestActivate()
        tryVerify(() => window.active, 5000, "the window takes the keyboard")
        tryVerify(() => findNamed(window.view, "formLayer0") !== null, 2000, "page 1 lays its fields")
        return window
    }

    function field(window, order) {
        const found = findNamed(window.view, "formField_" + order)
        verify(found, "the field " + order)
        return found
    }

    // Edit mode, begun by a tap on the field at `order`.
    function fillFrom(window, order) {
        mouseClick(field(window, order))
        tryVerify(() => window.editing, 2000, "editing began")
        tryVerify(() => testCase.focusedField(window) === order, 2000, "on the field tapped")
    }

    function test_each_page_lays_its_own_fields() {
        const window = openForm()
        const page = findNamed(window.view, "formLayer0")
        compare(page.items.length, 6, "page 1's widgets")
        const name = field(window, 0)
        compare(name.width, 300 * window.view.renderScale)
        compare(name.y, 162 * window.view.renderScale)
        verify(!findNamed(name, "formFieldFrame").visible, "no control while reading")
        compare(window.view.formLayer !== null, true)
    }

    function test_a_tap_on_a_field_while_reading_starts_editing_on_it() {
        const window = openForm()
        fillFrom(window, 0)
        compare(window.editor.calls, ["begin"])
        verify(findNamed(field(window, 0), "formFieldFrame").visible, "a control now")
        verify(window.editTyping, "the edit keys are the field's")
    }

    function test_typing_and_tab_fill_the_fields_in_order() {
        const window = openForm()
        fillFrom(window, 0)
        keyClick("a")
        keyClick("d")
        keyClick("a")
        keyClick(Qt.Key_Tab)
        tryVerify(() => window.editor.calls.indexOf("fillText:o8.0:ada") >= 0, 2000, "the text went on Tab")
        tryVerify(() => testCase.focusedField(window) === 1, 2000, "then the checkbox")
        keyClick(Qt.Key_Space)
        verify(window.editor.calls.indexOf("setChecked:o11.0:true") >= 0, "Space checks it")
        compare(window.reader.page, 1, "Space did not turn the page")
        tryVerify(() => findNamed(field(window, 1), "formCheckMark").visible)
        keyClick(Qt.Key_Tab)
        tryVerify(() => testCase.focusedField(window) === 2, 2000, "then the city")
        keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
        tryVerify(() => testCase.focusedField(window) === 1, 2000, "Shift+Tab goes back")
        compare(JSON.parse(window.editor.fields)[0].value, "ada", "the value as the editor has it")
        verify(window.editor.edited)
    }

    function test_tab_reaches_the_same_field_on_the_next_page() {
        const window = openForm()
        fillFrom(window, 4)
        keyClick(Qt.Key_Tab)
        tryVerify(() => findNamed(window.view, "formLayer1") !== null, 2000, "page 2 is built")
        tryVerify(() => testCase.focusedField(window) === 5, 2000, "the second widget of «copy»")
        keyClick(Qt.Key_Tab)
        tryVerify(() => testCase.focusedField(window) === 0, 2000, "round to the first field")
    }

    function test_a_field_calcita_only_shows_says_why() {
        const window = openForm()
        mouseClick(field(window, 6))
        compare(findNamed(window.chrome, "noticeText").text, testCase.radioNotice)
        verify(!window.editing, "nothing begins")
        fillFrom(window, 0)
        mouseClick(field(window, 6))
        compare(findNamed(window.chrome, "noticeText").text, testCase.radioNotice, "while editing too")
    }

    function test_a_checkbox_toggles_by_tap_and_undoes() {
        const window = openForm()
        fillFrom(window, 1)
        mouseClick(field(window, 1))
        verify(window.editor.calls.indexOf("setChecked:o11.0:true") >= 0)
        verify(JSON.parse(window.editor.fields)[1].checked)
        keyClick(Qt.Key_Z, Qt.ControlModifier)
        verify(!JSON.parse(window.editor.fields)[1].checked, "Ctrl+Z takes it back")
        verify(!findNamed(field(window, 1), "formCheckMark").visible)
    }

    function test_values_join_the_save_question() {
        const window = openForm()
        fillFrom(window, 3)
        keyClick("2")
        keyClick("8")
        keyClick(Qt.Key_Return)
        tryVerify(() => window.editor.edited, 2000, "Enter commits the field")
        compare(window.activeFocusItem.text, "28", "the accepted text stays in the field")
        compare(window.editor.calls[window.editor.calls.length - 1], "fillText:o17.0:28")
        window.leaveEditing()
        verify(window.saveQuestion.shown, "leaving with a value asks")
        mouseClick(findNamed(window.saveQuestion, "keepBothButton"))
        compare(window.editor.saves, [false])
    }

    function test_escape_commits_and_gives_the_keyboard_back_to_the_pages() {
        const window = openForm()
        fillFrom(window, 2)
        keyClick("m")
        keyClick(Qt.Key_Escape)
        verify(window.editor.calls.indexOf("fillText:o15.0:m") >= 0)
        compare(testCase.focusedField(window), -1)
        verify(window.view.activeFocus, "the pages have the keyboard")
    }

    function test_a_refused_value_gives_way_to_the_fields_own() {
        const window = openForm()
        fillFrom(window, 0)
        const input = window.activeFocusItem
        compare(input.objectName, "formText")
        input.text = "\u0141ukasz"
        keyClick(Qt.Key_Tab)
        compare(findNamed(window.chrome, "noticeText").text, "unencodable", "the editor says why")
        compare(input.text, "", "the field's own value again")
        verify(!window.editor.edited, "nothing joined the edit set")
        compare(JSON.parse(window.editor.fields)[0].value, "")
        tryVerify(() => testCase.focusedField(window) === 1, 2000, "and Tab still moves on")
    }

    function test_escape_on_a_checkbox_gives_the_keyboard_back_to_the_pages() {
        const window = openForm()
        fillFrom(window, 1)
        keyClick(Qt.Key_Escape)
        compare(testCase.focusedField(window), -1)
        verify(window.view.activeFocus, "the pages have the keyboard")
    }

    function test_a_value_with_a_line_break_is_not_rewritten_untouched() {
        const window = createTemporaryObject(documentComponent, testCase,
                                             { "documentKey": testCase.fixture,
                                               "pdf": testPdf })
        testCase.current = window
        const fields = JSON.parse(JSON.stringify(testCase.fields))
        fields[0].value = "one\ntwo"
        window.editor.seedFields = fields
        window.reader.fields = JSON.stringify(fields)
        tryVerify(() => window.reader.loaded, 5000)
        window.requestActivate()
        tryVerify(() => window.active, 5000)
        tryVerify(() => findNamed(window.view, "formLayer0") !== null, 2000)
        fillFrom(window, 0)
        keyClick(Qt.Key_Tab)
        tryVerify(() => testCase.focusedField(window) === 1, 2000)
        verify(window.editor.calls.every(call => !call.startsWith("fillText:")), String(window.editor.calls))
        verify(!window.editor.edited, "nothing to save")
    }

    function test_leaving_right_after_typing_keeps_the_value() {
        const window = openForm()
        fillFrom(window, 0)
        keyClick("x")
        keyClick(Qt.Key_E, Qt.ControlModifier)
        verify(window.editor.calls.indexOf("fillText:o8.0:x") >= 0, "committed as the field lost the keyboard")
        verify(window.saveQuestion.shown, "so leaving asks")
    }

    function test_drawing_and_highlighting_leave_the_fields_alone() {
        const window = openForm()
        fillFrom(window, 0)
        window.editTool = "ink"
        verify(!field(window, 0).enabled, "the pen draws over a field")
        window.editTool = "text"
        verify(field(window, 0).enabled)
    }
}
