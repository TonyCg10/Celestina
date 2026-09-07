import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The path pill as an editor. Three ways in, each with its own selection: a
// press on empty pill falls through to the field, so the caret lands under
// the pointer and a sweep selects; the current folder's crumb opens the editor
// with that one name selected; an ancestor crumb stays a link. Escape puts
// the crumbs back.
TestCase {
    id: testCase
    name: "PathPill"
    width: 600
    height: 200
    visible: true
    when: windowShown

    QtObject {
        id: controllerStub

        property string currentPath: "/home/prueba"
        property var pathCrumbs: ["/\t/", "/home\thome", "/home/prueba\tprueba"]
        property var openedKeys: []
        property var openedLocations: []
        property bool searchActive: false
        property bool searchRunning: false
        property string searchQuery: ""
        property string searchSummary: ""

        function openKey(key) { openedKeys = openedKeys.concat([key]) }
        function openLocation(location) { openedLocations = openedLocations.concat([location]) }
        function applyQuery(text) { }
        function closeSearch() { }
        function searchRecursive(text) { }
        function cancelSearch() { }
    }

    QtObject {
        id: hostWindowStub
        property real interfaceTextScale: 1.0
        property real interfaceIconScale: 1.0
    }

    TopBar {
        id: topBar
        x: 20
        y: 60
        width: 500
        height: CelestinaTheme.controlHeightLg
        controller: controllerStub
        activeView: null
        hostWindow: hostWindowStub
        overlayParent: testCase
        pathMenu: null
    }

    readonly property Item pathPill: topBar.children[0]
    function field() { return findChild(topBar, "locationField") }
    function crumb(index) { return findChild(topBar, "crumb-" + index) }

    function init() {
        controllerStub.openedKeys = []
        controllerStub.openedLocations = []
        keyClick(Qt.Key_Escape)
        wait(0)
    }

    function test_a_press_on_empty_pill_edits_at_the_pointer_and_a_sweep_selects() {
        const y = pathPill.height / 2
        mousePress(pathPill, 40, y, Qt.LeftButton)
        tryCompare(pathPill, "editing", true)
        const editor = field()
        verify(editor.activeFocus, "the field did not take focus on the press")
        compare(editor.text, "/home/prueba")
        compare(editor.selectedText, "", "a press must place the caret, not select everything")
        mouseMove(pathPill, 60, y)
        mouseMove(pathPill, 140, y)
        mouseRelease(pathPill, 140, y, Qt.LeftButton)
        verify(editor.selectedText.length > 0, "the sweep selected nothing: the press did not reach the field")
        verify(editor.text.indexOf(editor.selectedText) >= 0)
    }

    function test_the_current_crumb_opens_the_editor_on_its_own_name() {
        const last = crumb(2)
        verify(last !== null, "no current crumb")
        mouseClick(last, last.width - 8, last.height / 2, Qt.LeftButton)
        tryCompare(pathPill, "editing", true)
        compare(field().selectedText, "prueba")
        compare(controllerStub.openedKeys.length, 0, "the current folder is not a link")
    }

    function test_an_ancestor_crumb_stays_a_link() {
        const home = crumb(1)
        verify(home !== null, "no ancestor crumb")
        mouseClick(home, home.width - 8, home.height / 2, Qt.LeftButton)
        wait(0)
        compare(controllerStub.openedKeys, ["/home"])
        compare(pathPill.editing, false)
    }

    function test_escape_puts_the_crumbs_back() {
        mousePress(pathPill, 40, pathPill.height / 2, Qt.LeftButton)
        mouseRelease(pathPill, 40, pathPill.height / 2, Qt.LeftButton)
        tryCompare(pathPill, "editing", true)
        keyClick(Qt.Key_Escape)
        tryCompare(pathPill, "editing", false)
        verify(crumb(2).visible)
    }
}
