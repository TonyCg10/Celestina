import QtQuick
import QtTest 1.3
import org.celestina.selenita 1.0

// The whole window, over QML stand-ins for the Rust types: it constructs,
// starts its activation adapter, shows the three cards in order, and a second
// launch brings it forward.
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

    function test_the_window_constructs_with_three_cards() {
        const window = createTemporaryObject(mainComponent, testCase)
        verify(window)
        tryVerify(function() { return window.visible })
        const names = ["captureCard", "recordingCard", "historyCard"]
        const titles = [qsTr("Captura"), qsTr("Grabación"), qsTr("Historial")]
        let previousBottom = -1
        for (let i = 0; i < names.length; i += 1) {
            const card = findNamed(window.contentItem, names[i])
            verify(card, names[i])
            verify(card.visible, names[i] + " is shown")
            compare(findNamed(card, "sectionLabel").text, titles[i].toUpperCase())
            verify(card.y > previousBottom, names[i] + " stacks below the one before")
            previousBottom = card.y + card.height
        }
    }

    function test_the_activation_adapter_starts_once() {
        const window = createTemporaryObject(mainComponent, testCase)
        compare(window.activation.starts, 1)
    }

    function test_a_second_launch_raises_the_window() {
        const window = createTemporaryObject(mainComponent, testCase)
        tryVerify(function() { return window.visible })
        window.hide()
        verify(!window.visible)
        window.activation.raiseRequested()
        verify(window.visible)
    }
}
