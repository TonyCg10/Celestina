import QtQuick
import QtTest 1.3
import QtQuick.Pdf
import org.celestina.calcita 1.0

// The document window's keys over the three-page fixture: pages, ends, the
// page field, zoom and open.
TestCase {
    id: testCase
    name: "Keyboard"
    when: windowShown

    readonly property string fixture: String(Qt.resolvedUrl("../fixtures/three-pages.pdf"))
                                      .substring("file://".length)

    // Owned here, as Main owns the application's: it outlives each window.
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

    function init() {
        CalcitaController.reset()
    }

    // The window of the test, closed the way a person closes it, so its
    // document is emptied before the test case destroys it.
    property var current: null

    function cleanup() {
        if (testCase.current !== null)
            testCase.current.close()
        testCase.current = null
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

    function test_page_down_space_and_page_up_walk_the_pages() {
        const window = openFixture()
        keyClick(Qt.Key_PageDown)
        tryCompare(window.reader, "page", 2)
        keyClick(Qt.Key_Space)
        tryCompare(window.reader, "page", 3)
        keyClick(Qt.Key_PageDown)
        compare(window.reader.page, 3, "the last page stays")
        keyClick(Qt.Key_PageUp)
        tryCompare(window.reader, "page", 2)
        keyClick(Qt.Key_Space, Qt.ShiftModifier)
        tryCompare(window.reader, "page", 1)
    }

    function test_home_and_end_reach_the_ends() {
        const window = openFixture()
        keyClick(Qt.Key_End)
        tryCompare(window.reader, "page", 3)
        tryCompare(window.view, "currentPage", 2)
        keyClick(Qt.Key_Home)
        tryCompare(window.reader, "page", 1)
    }

    function test_ctrl_g_focuses_the_page_field() {
        const window = openFixture()
        keyClick(Qt.Key_G, Qt.ControlModifier)
        const field = findNamed(window.contentItem, "pageField")
        tryVerify(() => field.activeFocus)
        keyClick(Qt.Key_2)
        keyClick(Qt.Key_Return)
        tryCompare(window.reader, "page", 2)
        verify(!field.activeFocus, "Enter hands the focus back to the pages")
        tryCompare(field, "text", "2 / 3")
    }

    function test_zoom_keys() {
        const window = openFixture()
        keyClick(Qt.Key_0, Qt.ControlModifier)
        compare(window.reader.zoomFactor, 1)
        keyClick(Qt.Key_Plus, Qt.ControlModifier)
        compare(window.reader.zoomFactor, 1.1)
        keyClick(Qt.Key_Equal, Qt.ControlModifier)
        compare(window.reader.zoomFactor, 1.25)
        keyClick(Qt.Key_Minus, Qt.ControlModifier)
        compare(window.reader.zoomFactor, 1.1)
        keyClick(Qt.Key_2, Qt.ControlModifier)
        compare(window.reader.zoomMode, "fitPage")
        keyClick(Qt.Key_1, Qt.ControlModifier)
        compare(window.reader.zoomMode, "fitWidth")
    }

    function test_ctrl_o_asks_for_the_chooser() {
        const window = openFixture()
        const asked = createTemporaryQmlObject(
            "import QtTest; SignalSpy { signalName: \"openRequested\" }", testCase)
        asked.target = window
        keyClick(Qt.Key_O, Qt.ControlModifier)
        compare(asked.count, 1)
    }

    function test_space_in_the_page_field_types_and_does_not_scroll() {
        const window = openFixture()
        keyClick(Qt.Key_G, Qt.ControlModifier)
        const field = findNamed(window.contentItem, "pageField")
        tryVerify(() => field.activeFocus)
        const before = window.view.contentY
        keyClick(Qt.Key_Space)
        compare(window.reader.page, 1)
        compare(window.view.contentY, before)
        compare(field.text, " ")
    }
}
