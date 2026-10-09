// language-contract: allow-non-english (the Spanish notices the window shows, compared verbatim)
import QtQuick
import QtTest 1.3
import QtQuick.Pdf
import org.celestina.calcita 1.0

// One document window over the three-page fixture, with QtPdf for real: it
// loads, goes to a page, zooms along the ladder and follows a scroll in the
// page field.
TestCase {
    id: testCase
    name: "Document"
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

    // The Flickable inside QtPdf's view, which a wheel or a drag moves.
    function findFlickable(node) {
        if (node.contentY !== undefined && node.rowSpacing !== undefined)
            return node
        const kids = node.children !== undefined ? node.children : []
        let found = null
        let i = 0
        while (found === null && i < kids.length) {
            found = findFlickable(kids[i])
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
        verify(window)
        tryVerify(() => window.reader.loaded, 5000, "the fixture loads")
        return window
    }

    function test_the_document_loads_with_three_pages() {
        const window = openFixture()
        compare(window.reader.pageCount, 3)
        compare(window.reader.page, 1)
        compare(window.reader.documentName, "three-pages.pdf")
        compare(window.title, "three-pages.pdf")
        compare(findNamed(window.contentItem, "documentName").text, "three-pages.pdf")
    }

    function test_go_to_three_scrolls_to_the_third_page() {
        const window = openFixture()
        verify(window.reader.goTo("3"))
        tryCompare(window.view, "currentPage", 2)
        compare(window.reader.page, 3)
        tryCompare(findNamed(window.contentItem, "pageField"), "text", "3 / 3")
    }

    function test_a_page_outside_the_document_is_a_notice() {
        const window = openFixture()
        verify(!window.reader.goTo("9"))
        compare(window.reader.page, 1)
        const notice = findNamed(window.contentItem, "notice")
        tryVerify(() => notice.visible)
        compare(notice.text, "Esa página no existe en este documento.")
    }

    function test_the_zoom_buttons_walk_the_ladder() {
        const window = openFixture()
        compare(window.reader.zoomMode, "fitWidth")
        window.reader.setZoom(1)
        compare(window.reader.zoomFactor, 1)
        compare(window.view.renderScale, 1)
        mouseClick(findNamed(window.contentItem, "zoomInButton"))
        compare(window.reader.zoomFactor, 1.1)
        mouseClick(findNamed(window.contentItem, "zoomInButton"))
        compare(window.reader.zoomFactor, 1.25)
        compare(window.view.renderScale, 1.25)
        mouseClick(findNamed(window.contentItem, "zoomOutButton"))
        compare(window.reader.zoomFactor, 1.1)
        compare(findNamed(window.contentItem, "zoomLabel").text, "110 %")
    }

    function test_fit_page_makes_the_page_fit_the_view() {
        const window = openFixture()
        mouseClick(findNamed(window.contentItem, "fitPageButton"))
        compare(window.reader.zoomMode, "fitPage")
        verify(842 * window.view.renderScale <= window.view.height)
        mouseClick(findNamed(window.contentItem, "fitWidthButton"))
        compare(window.reader.zoomMode, "fitWidth")
        verify(595 * window.view.renderScale <= window.view.width)
    }

    function test_the_page_field_follows_a_scroll() {
        const window = openFixture()
        window.reader.setZoom(1)
        const flickable = findFlickable(window.view)
        verify(flickable, "the view's flickable")
        flickable.contentY = 842 + 100
        tryCompare(window.reader, "page", 2)
        tryCompare(findNamed(window.contentItem, "pageField"), "text", "2 / 3")
    }

    function test_the_page_field_takes_a_page() {
        const window = openFixture()
        const field = findNamed(window.contentItem, "pageField")
        window.bar.goToRequested("fin")
        tryCompare(window.view, "currentPage", 2)
        tryCompare(field, "text", "3 / 3")
    }

    function test_the_window_remembers_where_it_was_left() {
        const window = openFixture()
        window.reader.setZoom(1.5)
        window.reader.goTo("2")
        tryCompare(window.view, "currentPage", 1)
        window.close()
        compare(CalcitaController.restoredPage(testCase.fixture), 2)
        compare(CalcitaController.restoredZoom(testCase.fixture), "free:1.5")
        compare(CalcitaController.recentNames[0], "three-pages.pdf")
    }
}
