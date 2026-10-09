import QtQuick
import QtTest 1.3
import org.celestina.calcita 1.0

// The whole window, over QML stand-ins for the Rust types: it constructs,
// starts its activation adapter, shows the empty state, and a second launch
// brings it forward.
TestCase {
    id: testCase
    name: "Main"
    when: windowShown

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
}
