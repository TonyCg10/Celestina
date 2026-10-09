import QtQuick
import QtTest 1.3
import QtQuick.Pdf
import org.celestina.calcita 1.0

// The page view's cost with a long document: 300 empty A4 pages open, scroll
// to the end and back within a time budget, and only the pages near the
// viewport build their image.
TestCase {
    id: testCase
    name: "LongDocument"
    when: windowShown

    readonly property string fixture: String(Qt.resolvedUrl("../fixtures/many-pages.pdf"))
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

    function builtImages(window) {
        let built = 0
        for (let page = 0; page < window.view.count; page += 1) {
            const loader = findNamed(window.view, "page" + page)
            if (loader !== null && loader.item !== null)
                built += 1
        }
        return built
    }

    function test_three_hundred_pages_open_and_scroll_within_budget() {
        const started = Date.now()
        const window = openFixture()
        compare(window.reader.pageCount, 300)
        window.reader.setZoom(1)
        const flick = window.view
        for (let step = 0; step < 60; step += 1)
            flick.contentY = (flick.contentHeight - flick.height) * step / 59
        tryCompare(window.reader, "page", 300)
        for (let step = 59; step >= 0; step -= 1)
            flick.contentY = (flick.contentHeight - flick.height) * step / 59
        tryCompare(window.reader, "page", 1)
        const elapsed = Date.now() - started
        console.log("long document:", elapsed, "ms,", builtImages(window), "images built")
        verify(elapsed < 2000, "open, to the end and back took " + elapsed + " ms")
        verify(window.view.builtPages <= 6, "built pages: " + window.view.builtPages)
        verify(builtImages(window) <= 6, "built images: " + builtImages(window))
    }

    function test_a_jump_to_the_last_page_lands_on_it() {
        const window = openFixture()
        verify(window.reader.goTo("fin"))
        tryCompare(window.view, "currentPage", 299)
        verify(window.view.firstNear >= 290)
    }
}
