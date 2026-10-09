// language-contract: allow-non-english (the Spanish notices the window shows, compared verbatim)
import QtQuick
import QtTest 1.3
import org.celestina.calcita 1.0

// The whole first window, over QML stand-ins for the Rust types: it
// constructs with the empty state, starts its activation adapter, and the
// suite's joins reach it — `Open` opens one window per document, a refused
// drop is a notice, a recent document reopens.
TestCase {
    id: testCase
    name: "Main"
    when: windowShown

    readonly property string fixture: String(Qt.resolvedUrl("../fixtures/three-pages.pdf"))
                                      .substring("file://".length)

    Component {
        id: mainComponent
        Main {}
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


    function test_the_window_constructs_with_the_empty_state() {
        const window = createTemporaryObject(mainComponent, testCase)
        verify(window)
        tryVerify(function() { return window.visible })
        const empty = findNamed(window.contentItem, "emptyState")
        verify(empty, "the empty-state card")
        verify(empty.visible)
        compare(findNamed(empty, "emptyTitle").text, "Sin documento")
        const open = findNamed(empty, "openButton")
        verify(open.visible)
        compare(open.text, "Abrir…")
        verify(!findNamed(empty, "recentCard").visible, "no recents yet")
    }

    function test_the_activation_adapter_starts_once() {
        const window = createTemporaryObject(mainComponent, testCase)
        compare(window.activation.starts, 1)
    }

    function test_the_open_button_asks_for_a_document() {
        const window = createTemporaryObject(mainComponent, testCase)
        const empty = findNamed(window.contentItem, "emptyState")
        const asked = createTemporaryQmlObject(
            "import QtTest; SignalSpy { signalName: \"openRequested\" }", testCase)
        asked.target = empty
        mouseClick(findNamed(empty, "openButton"))
        compare(asked.count, 1)
    }

    function test_open_with_one_path_replaces_the_empty_state() {
        const window = createTemporaryObject(mainComponent, testCase)
        window.activation.openRequested([testCase.fixture])
        compare(window.documentWindows.count, 1)
        verify(!window.visible, "the empty window retires")
        const document = window.documentWindows.objectAt(0)
        compare(document.documentKey, testCase.fixture)
        tryVerify(() => document.reader.loaded, 5000)
        compare(document.reader.pageCount, 3)
        compare(window.front, document)
    }

    function test_open_with_several_paths_opens_a_window_each() {
        const window = createTemporaryObject(mainComponent, testCase)
        window.activation.openRequested(["/tmp/a.pdf", "/tmp/b.pdf"])
        compare(window.documentWindows.count, 2)
        compare(window.documentWindows.objectAt(0).documentKey, "/tmp/a.pdf")
        compare(window.documentWindows.objectAt(1).documentKey, "/tmp/b.pdf")
    }

    function test_an_open_document_is_raised_not_opened_twice() {
        const window = createTemporaryObject(mainComponent, testCase)
        const raised = createTemporaryQmlObject(
            "import QtTest; SignalSpy { signalName: \"raiseDocument\" }", testCase)
        raised.target = CalcitaController
        window.activation.openRequested([testCase.fixture])
        window.activation.openRequested([testCase.fixture])
        compare(window.documentWindows.count, 1)
        compare(raised.count, 1)
    }

    function test_closing_a_document_window_drops_it() {
        const window = createTemporaryObject(mainComponent, testCase)
        window.activation.openRequested(["/tmp/a.pdf", "/tmp/b.pdf"])
        window.documentWindows.objectAt(0).close()
        tryCompare(window.documentWindows, "count", 1)
        compare(CalcitaController.documents, ["/tmp/b.pdf"])
    }

    function test_a_dropped_non_pdf_is_a_notice_and_no_document() {
        const window = createTemporaryObject(mainComponent, testCase)
        tryVerify(() => window.visible)
        CalcitaController.openDropped(["file:///tmp/foto.png"], "main")
        compare(window.documentWindows.count, 0)
        verify(window.visible)
        const notice = findNamed(window.contentItem, "notice")
        tryVerify(() => notice.visible)
        compare(notice.text, "Calcita solo abre documentos PDF.")
    }

    function test_a_dropped_pdf_opens_it() {
        const window = createTemporaryObject(mainComponent, testCase)
        CalcitaController.openDropped(["file://" + testCase.fixture], "main")
        compare(window.documentWindows.count, 1)
    }

    function test_a_recent_document_reopens_from_the_empty_state() {
        CalcitaController.remember(testCase.fixture, 2, "width")
        const window = createTemporaryObject(mainComponent, testCase)
        const empty = findNamed(window.contentItem, "emptyState")
        verify(findNamed(empty, "recentCard").visible)
        const row = findNamed(empty, "recentRow0")
        verify(row)
        waitForRendering(empty)
        mouseClick(row)
        compare(window.documentWindows.count, 1)
        const document = window.documentWindows.objectAt(0)
        tryVerify(() => document.reader.loaded, 5000)
        tryCompare(document.reader, "page", 2)
    }

    function test_a_refused_drop_shows_on_the_window_it_was_dropped_on() {
        const window = createTemporaryObject(mainComponent, testCase)
        window.activation.openRequested(["/tmp/a.pdf", "/tmp/b.pdf"])
        const first = window.documentWindows.objectAt(0)
        const second = window.documentWindows.objectAt(1)
        verify(!first.active, "the window dropped on is not the active one")
        CalcitaController.openDropped(["file:///tmp/foto.png"], first.documentKey)
        const notice = findNamed(first.contentItem, "notice")
        tryVerify(() => notice.visible)
        compare(notice.text, "Calcita solo abre documentos PDF.")
        // The other window only says its own missing file could not be read.
        compare(findNamed(second.contentItem, "notice").text, "No se pudo abrir el documento.")
    }

    function test_a_notice_no_window_asked_for_shows_on_the_front_window() {
        const window = createTemporaryObject(mainComponent, testCase)
        window.activation.openRequested(["/tmp/a.pdf", testCase.fixture])
        const front = window.documentWindows.objectAt(1)
        compare(window.front, front)
        tryCompare(front.reader, "pageCount", 3, 5000)
        window.activation.openRequested(["/tmp/foto.png"])
        const notice = findNamed(front.contentItem, "notice")
        tryCompare(notice, "text", "Calcita solo abre documentos PDF.")
        tryVerify(() => notice.visible)
    }

    function test_activate_raises_the_last_focused_window() {
        const window = createTemporaryObject(mainComponent, testCase)
        window.activation.openRequested(["/tmp/a.pdf", "/tmp/b.pdf"])
        const first = window.documentWindows.objectAt(0)
        compare(window.front, window.documentWindows.objectAt(1), "the newest at first")
        first.requestActivate()
        tryVerify(() => first.active)
        compare(window.front, first)
        verify(first.front)
        window.openChooser(first.documentKey)
        compare(window.fileDialog.parentWindow, first, "the chooser sits over it")
        window.fileDialog.close()
        first.close()
        tryCompare(window.documentWindows, "count", 1)
        compare(window.fileDialog.parentWindow, null)
        compare(window.front, window.documentWindows.objectAt(0))
    }

    // Closing a window while QtPdf may still be rendering its pages is safe:
    // the document is emptied first and the window destroyed a turn later.
    function test_closing_right_after_loading_twenty_times() {
        const window = createTemporaryObject(mainComponent, testCase)
        let round = 0
        while (round < 20) {
            window.activation.openRequested([testCase.fixture])
            tryCompare(window.documentWindows, "count", 1)
            const document = window.documentWindows.objectAt(0)
            tryCompare(document.reader, "pageCount", 3, 5000)
            document.close()
            tryCompare(window.documentWindows, "count", 0)
            round += 1
        }
        compare(CalcitaController.documents, [])
    }

    // Between closing and its destruction a turn later, the window no longer
    // holds its document: opening the same path again opens a fresh window.
    function test_reopening_a_path_right_after_closing_it() {
        const window = createTemporaryObject(mainComponent, testCase)
        window.activation.openRequested([testCase.fixture])
        const first = window.documentWindows.objectAt(0)
        tryCompare(first.reader, "pageCount", 3, 5000)
        first.close()
        compare(window.front === first, false, "the closing window is not the front")
        CalcitaController.openPath(testCase.fixture, "")
        tryCompare(window.documentWindows, "count", 1)
        const second = window.documentWindows.objectAt(0)
        verify(second !== first)
        tryCompare(second.reader, "pageCount", 3, 5000)
        compare(CalcitaController.documents, [testCase.fixture])
    }
}
