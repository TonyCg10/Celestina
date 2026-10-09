// language-contract: allow-non-english (the Spanish counter and placeholder the card shows, compared verbatim)
import QtQuick
import QtTest 1.3
import QtQuick.Pdf
import org.celestina.calcita 1.0

// Search over the outline fixture, whose three pages read «Page one», «Page
// two» and «Page three»: the card opens with Ctrl+F, the hits are counted,
// Enter/Shift+Enter and F3/Shift+F3 walk them with the counter «n de N», the
// view follows the current hit, and Escape closes the card.
TestCase {
    id: testCase
    name: "Search"
    when: windowShown

    readonly property string fixture: String(Qt.resolvedUrl("../fixtures/outline.pdf"))
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

    function typeQuery(window, text) {
        keyClick(Qt.Key_F, Qt.ControlModifier)
        const field = findNamed(window.contentItem, "searchField")
        tryVerify(() => field.activeFocus, 2000, "the field takes the focus")
        for (const letter of text)
            keyClick(letter)
        return field
    }

    function test_ctrl_f_opens_the_card_and_page_finds_three_hits() {
        const window = openFixture()
        verify(!window.search.visible)
        typeQuery(window, "page")
        verify(window.search.visible)
        tryCompare(window.view.searchModel, "searchString", "page")
        tryCompare(window.view.searchModel, "count", 3, 5000)
    }

    function test_enter_and_shift_enter_walk_the_hits() {
        const window = openFixture()
        typeQuery(window, "page")
        tryCompare(window.view.searchModel, "count", 3, 5000)
        const counter = findNamed(window.contentItem, "hitCounter")
        compare(counter.text, "1 de 3", "the first hit is current at once")
        keyClick(Qt.Key_Return)
        compare(counter.text, "2 de 3")
        compare(window.view.searchModel.currentPage, 1)
        tryCompare(window.view, "currentPage", 1)
        keyClick(Qt.Key_Return)
        compare(counter.text, "3 de 3")
        keyClick(Qt.Key_Return)
        compare(counter.text, "1 de 3", "Enter goes round the end")
        keyClick(Qt.Key_Return, Qt.ShiftModifier)
        compare(counter.text, "3 de 3", "Shift+Enter goes round the start")
        tryCompare(window.view, "currentPage", 2)
        tryCompare(window.reader, "page", 3)
    }

    function test_f3_and_shift_f3_walk_the_hits() {
        const window = openFixture()
        typeQuery(window, "page")
        tryCompare(window.view.searchModel, "count", 3, 5000)
        const counter = findNamed(window.contentItem, "hitCounter")
        keyClick(Qt.Key_F3)
        keyClick(Qt.Key_F3)
        compare(counter.text, "3 de 3")
        keyClick(Qt.Key_F3, Qt.ShiftModifier)
        compare(counter.text, "2 de 3")
    }

    function test_the_current_hit_is_painted_on_its_page() {
        const window = openFixture()
        typeQuery(window, "page")
        tryCompare(window.view.searchModel, "count", 3, 5000)
        keyClick(Qt.Key_Return)
        const page = findNamed(window.view, "page1")
        tryVerify(() => page.item !== null)
        const mark = findNamed(page.item, "currentHit")
        verify(mark.visible)
        compare(page.item.markedHits, 1, "page two's hit is marked")
        verify(window.view.searchModel.currentResultBoundingPolygons.length > 0)
    }

    function test_hits_arriving_later_leave_the_view_alone() {
        const window = openFixture()
        typeQuery(window, "page")
        tryCompare(window.view.searchModel, "count", 3, 5000)
        window.view.contentY = 500
        // QtPdf reports more hits as it reads on; the reader stays put.
        window.view.searchModel.countChanged()
        compare(window.view.contentY, 500)
    }

    function test_typing_waits_before_searching() {
        const window = openFixture()
        typeQuery(window, "pa")
        compare(window.view.searchModel.searchString, "", "not searched while typing")
        tryCompare(window.view.searchModel, "searchString", "pa", 1000)
    }

    function test_closing_while_typing_searches_nothing() {
        const window = openFixture()
        const before = window.view.contentY
        typeQuery(window, "page")
        keyClick(Qt.Key_Escape)
        wait(200)
        compare(window.view.searchModel.searchString, "")
        compare(window.view.contentY, before)
        compare(window.search.text, "")
    }

    function test_a_word_not_in_the_document_says_so() {
        const window = openFixture()
        typeQuery(window, "zebra")
        const counter = findNamed(window.contentItem, "hitCounter")
        tryCompare(counter, "text", "Sin resultados")
        keyClick(Qt.Key_Return)
        compare(counter.text, "Sin resultados")
    }

    function test_escape_closes_the_card_and_clears_the_search() {
        const window = openFixture()
        typeQuery(window, "page")
        keyClick(Qt.Key_Escape)
        verify(!window.search.visible)
        compare(window.view.searchModel.searchString, "")
        tryVerify(() => window.view.activeFocus)
    }

    function test_the_bar_button_opens_and_closes_the_card() {
        const window = openFixture()
        const button = findNamed(window.contentItem, "searchButton")
        compare(button.Accessible.name, "Buscar")
        mouseClick(button)
        verify(window.search.visible)
        mouseClick(button)
        verify(!window.search.visible)
    }
}
