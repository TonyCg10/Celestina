import QtQuick
import QtTest 1.3
import QtQuick.Pdf
import org.celestina.calcita 1.0

// The dark reading mode over the three-page fixture: the bar's button and
// Ctrl+I turn the page view's layer effect on and off, the fade honours
// reduced motion, and the mode is remembered per document and restored.
TestCase {
    id: testCase
    name: "ReadingMode"
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
        CelestinaTheme.reducedMotion = false
    }

    function cleanup() {
        if (testCase.current !== null)
            testCase.current.close()
        testCase.current = null
        CelestinaTheme.reducedMotion = false
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

    function test_the_reading_mode_starts_off() {
        const window = openFixture()
        verify(!window.readingDark)
        verify(!window.view.layer.enabled, "no layer while reading as drawn")
    }

    function test_the_bar_button_toggles_the_effect() {
        const window = openFixture()
        const button = findNamed(window.bar, "readingButton")
        verify(button)
        compare(button.Accessible.name, "Lectura oscura")
        mouseClick(button)
        verify(window.readingDark)
        tryVerify(() => window.view.layer.enabled)
        tryCompare(window, "readingAmount", 1, 2000)
        compare(button.Accessible.name, "Lectura clara")
        mouseClick(button)
        verify(!window.readingDark)
        tryVerify(() => !window.view.layer.enabled, 2000, "the layer goes once faded out")
    }

    function test_ctrl_i_toggles_the_effect() {
        const window = openFixture()
        keyClick(Qt.Key_I, Qt.ControlModifier)
        verify(window.readingDark)
        tryVerify(() => window.view.layer.enabled)
        keyClick(Qt.Key_I, Qt.ControlModifier)
        verify(!window.readingDark)
        tryVerify(() => !window.view.layer.enabled, 2000)
    }

    function test_the_fade_runs_without_reduced_motion() {
        const window = openFixture()
        verify(window.readingFade.enabled)
        window.toggleReadingMode()
        verify(window.readingAmount < 1, "the amount fades in")
        tryCompare(window, "readingAmount", 1, 2000)
    }

    function test_reduced_motion_disables_the_fade() {
        CelestinaTheme.reducedMotion = true
        const window = openFixture()
        verify(!window.readingFade.enabled)
        window.toggleReadingMode()
        compare(window.readingAmount, 1, "at once")
        verify(window.view.layer.enabled)
        window.toggleReadingMode()
        compare(window.readingAmount, 0, "at once")
        verify(!window.view.layer.enabled)
    }

    function test_the_mode_is_remembered_and_restored() {
        const window = openFixture()
        window.toggleReadingMode()
        verify(CalcitaController.restoredDark(testCase.fixture), "remembered on toggle")
        window.close()
        testCase.current = null
        wait(0)
        const again = openFixture()
        tryVerify(() => again.readingDark, 2000, "restored on open")
        tryVerify(() => again.view.layer.enabled)
    }
}
