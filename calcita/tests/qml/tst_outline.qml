// language-contract: allow-non-english (the Spanish names and words the window shows, compared verbatim)
import QtQuick
import QtTest 1.3
import QtQuick.Pdf
import org.celestina.calcita 1.0

// The outline, the links and the copy over the outline fixture: two
// bookmarks (Introduction → page 1, Chapter two → page 2), an internal link
// on page 1 to page 3 and an external one to https://example.org/guide.
TestCase {
    id: testCase
    name: "Outline"
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

    // The outline has its two bookmarks; the document is Ready first.
    function waitRows(window) {
        tryCompare(testPdf, "status", PdfDocument.Ready, 15000)
        tryCompare(window.outline, "rowCount", 2, 15000)
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

    function pageItem(window, page) {
        const loader = findNamed(window.view, "page" + page)
        tryVerify(() => loader.item !== null, 5000, "the page is built")
        return loader
    }

    function test_f9_shows_the_two_bookmarks() {
        const window = openFixture()
        verify(!window.outline.visible)
        keyClick(Qt.Key_F9)
        verify(window.outline.visible)
        waitRows(window)
        compare(window.outline.title(0), "Introduction")
        compare(window.outline.title(1), "Chapter two")
        const row = findNamed(window.outline, "outlineRow1")
        verify(row)
        compare(row.Accessible.name, "Chapter two")
    }

    function test_enter_follows_the_current_bookmark() {
        const window = openFixture()
        keyClick(Qt.Key_F9)
        waitRows(window)
        tryVerify(() => findNamed(window.outline, "outlineTree").activeFocus)
        keyClick(Qt.Key_Down)
        keyClick(Qt.Key_Return)
        tryCompare(window.view, "currentPage", 1)
        compare(window.reader.page, 2)
        keyClick(Qt.Key_Up)
        keyClick(Qt.Key_Return)
        tryCompare(window.view, "currentPage", 0)
    }

    function test_a_click_on_a_bookmark_navigates() {
        const window = openFixture()
        mouseClick(findNamed(window.contentItem, "outlineButton"))
        waitRows(window)
        const row = findNamed(window.outline, "outlineRow1")
        mouseClick(row)
        tryCompare(window.view, "currentPage", 1)
    }

    function test_escape_closes_the_outline() {
        const window = openFixture()
        keyClick(Qt.Key_F9)
        verify(window.outline.visible)
        keyClick(Qt.Key_Escape)
        verify(!window.outline.visible)
        tryVerify(() => window.view.activeFocus)
    }

    function test_the_internal_link_goes_to_page_three() {
        const window = openFixture()
        const link = findNamed(pageItem(window, 0).item, "link0_0")
        verify(link)
        compare(link.Accessible.name, "Ir a la página 3")
        mouseClick(link)
        tryCompare(window.view, "currentPage", 2)
        tryCompare(window.reader, "page", 3)
        verify(!window.linkPill.asking)
    }

    function test_an_external_link_asks_first() {
        const window = openFixture()
        const link = findNamed(pageItem(window, 0).item, "link0_1")
        mouseClick(link)
        verify(window.linkPill.asking)
        tryCompare(window.linkPill, "opacity", 1)
        compare(findNamed(window.linkPill, "linkHost").text, "example.org")
        compare(window.linkPill.Accessible.name, "Abrir enlace externo example.org")
        compare(CalcitaController.openedLinks.length, 0, "nothing opened before the answer")
        mouseClick(findNamed(window.linkPill, "openLinkButton"))
        compare(CalcitaController.openedLinks, ["https://example.org/guide"])
        verify(!window.linkPill.asking)
    }

    function test_an_external_link_can_be_refused() {
        const window = openFixture()
        mouseClick(findNamed(pageItem(window, 0).item, "link0_1"))
        verify(window.linkPill.asking)
        keyClick(Qt.Key_Escape)
        verify(!window.linkPill.asking)
        compare(CalcitaController.openedLinks.length, 0)
    }

    function test_a_downward_drag_at_150_selects_several_lines() {
        const window = openFixture()
        window.reader.setZoom(1.5)
        const page = pageItem(window, 0).item
        // A drag never flicks the view: that is what keeps it a selection.
        compare(window.view.interactive, false)
        const before = window.view.contentY
        // From «Page one» down to the «Visit the guide» line (baseline at
        // 182 pt), a vertical drag inside the page.
        mouseDrag(page, 74 * 1.5, 74 * 1.5, 40 * 1.5, 104 * 1.5)
        tryVerify(() => window.view.selectedText.indexOf("Page one") >= 0
                  && window.view.selectedText.indexOf("Go to the end") >= 0, 5000,
                  "the drag selects across the lines: " + window.view.selectedText)
        compare(window.view.contentY, before, "the drag does not scroll")
    }

    function test_a_drag_selects_text_and_ctrl_c_copies_it() {
        const window = openFixture()
        window.reader.setZoom(1)
        const page = pageItem(window, 0).item
        // «Page one» starts 72 pt from the left, its baseline 82 pt down; the drag
        // starts on its first letter.
        mouseDrag(page, 74, 74, 100, 0)
        tryVerify(() => window.view.selectedText.indexOf("Page one") >= 0, 5000,
                  "the drag selects the heading")
        keyClick(Qt.Key_C, Qt.ControlModifier)
        compare(CalcitaController.clipboard, window.view.selectedText)
    }
}
