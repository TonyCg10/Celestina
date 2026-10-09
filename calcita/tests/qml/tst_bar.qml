import QtQuick
import QtTest 1.3
import org.celestina.calcita 1.0
import "../../qml/components"

// The document bar on its own: the page field shows «n / N» and hands what
// is typed to its owner; the zoom controls report and show the factor.
TestCase {
    id: testCase
    name: "Bar"
    when: windowShown
    // qmltestrunner hides the test case until told; the bar is clicked.
    visible: true
    width: 900
    height: 200

    Component {
        id: barComponent
        DocumentBar {
            name: "informe.pdf"
            page: 2
            pageCount: 3
            zoomMode: "fitWidth"
            zoomFactor: 1.25
        }
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

    function spy(target, signalName) {
        const watcher = createTemporaryQmlObject(
            "import QtTest; SignalSpy { signalName: \"" + signalName + "\" }", testCase)
        watcher.target = target
        return watcher
    }

    function test_the_page_field_shows_current_of_total() {
        const bar = createTemporaryObject(barComponent, testCase)
        compare(findNamed(bar, "pageField").text, "2 / 3")
        bar.page = 3
        compare(findNamed(bar, "pageField").text, "3 / 3")
        compare(findNamed(bar, "documentName").text, "informe.pdf")
    }

    function test_typing_a_page_asks_for_it() {
        const bar = createTemporaryObject(barComponent, testCase)
        const asked = spy(bar, "goToRequested")
        bar.focusPageField()
        const field = findNamed(bar, "pageField")
        verify(field.activeFocus)
        keyClick(Qt.Key_Backspace)
        keyClick("1")
        keyClick(Qt.Key_Return)
        compare(asked.count, 1)
        compare(asked.signalArguments[0][0], "1")
    }

    function test_the_field_shows_the_page_again_when_left() {
        const bar = createTemporaryObject(barComponent, testCase)
        bar.focusPageField()
        keyClick("x")
        bar.forceActiveFocus()
        testCase.forceActiveFocus()
        compare(findNamed(bar, "pageField").text, "2 / 3")
    }

    function test_the_zoom_controls_report() {
        const bar = createTemporaryObject(barComponent, testCase)
        compare(findNamed(bar, "zoomLabel").text, "125 %")
        waitForRendering(bar)
        const zoomIn = spy(bar, "zoomInRequested")
        const zoomOut = spy(bar, "zoomOutRequested")
        const width = spy(bar, "fitWidthRequested")
        const page = spy(bar, "fitPageRequested")
        const open = spy(bar, "openRequested")
        mouseClick(findNamed(bar, "zoomInButton"))
        mouseClick(findNamed(bar, "zoomOutButton"))
        mouseClick(findNamed(bar, "fitWidthButton"))
        mouseClick(findNamed(bar, "fitPageButton"))
        mouseClick(findNamed(bar, "barOpenButton"))
        compare([zoomIn.count, zoomOut.count, width.count, page.count, open.count],
                [1, 1, 1, 1, 1])
    }

    function test_every_control_has_a_name() {
        const bar = createTemporaryObject(barComponent, testCase)
        const names = ["pageField", "zoomInButton", "zoomOutButton", "fitWidthButton",
                       "fitPageButton", "barOpenButton"]
        names.forEach(name => verify(findNamed(bar, name).Accessible.name.length > 0, name))
    }
}
