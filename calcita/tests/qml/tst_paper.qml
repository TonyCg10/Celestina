import QtQuick
import QtTest 1.3
import QtQuick.Pdf
import org.celestina.calcita 1.0

// Every page is drawn on opaque paper: QtPdf renders a page with a clear
// background, so the paper is what keeps the ink off the window's glass. The
// sheet lies under the image, the hits, the selection and the links, and the
// reading mode's layer covers it with them.
TestCase {
    id: testCase
    name: "Paper"
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
    }

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
        tryVerify(() => window.view.builtPages > 0, 5000, "the near pages are built")
        return window
    }

    function test_the_page_is_opaque_paper_under_the_image() {
        const window = openFixture()
        const page = findNamed(window.view, "page0")
        verify(page, "the first page's shell")
        tryVerify(() => page.item !== null, 2000, "the first page is built")
        const paper = findNamed(page.item, "paper")
        verify(paper, "the paper sheet exists")
        compare(paper.color.a, 1, "the paper is opaque")
        compare(paper.color, window.view.paper, "the paper is the view's token")
        compare(window.view.paper, CelestinaTheme.iconSheet, "the token is the suite's sheet")
        compare(paper.width, page.item.width)
        compare(paper.height, page.item.height)
        verify(!window.view.layer.enabled, "as drawn, no layer")
    }

    function test_the_paper_lies_under_everything_else() {
        const window = openFixture()
        const page = findNamed(window.view, "page0")
        tryVerify(() => page.item !== null, 2000)
        const kids = page.item.children
        verify(kids.length > 1, "the image and the overlays follow the paper")
        compare(kids[0].objectName, "paper", "the paper is the lowest child")
        verify(kids[1] instanceof Image, "the page image is drawn over the paper")
        verify(findNamed(page.item, "currentHit"), "the hit overlay is above the paper")
    }

    function test_the_reading_layer_covers_the_paper() {
        const window = openFixture()
        const page = findNamed(window.view, "page0")
        tryVerify(() => page.item !== null, 2000)
        const paper = findNamed(page.item, "paper")
        window.toggleReadingMode()
        tryVerify(() => window.view.layer.enabled, 2000, "the layer is on")
        compare(paper.color.a, 1, "the sheet stays opaque under the layer")
        // The layer is the view's, so it covers whatever the view holds.
        let node = paper
        while (node !== null && node !== window.view)
            node = node.parent
        verify(node === window.view, "the sheet lies inside the layered view")
    }
}
