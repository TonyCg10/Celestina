import QtQuick
import QtTest 1.3
import org.celestina.cuprita 1.0
import "../../qml/components"

// The section strip: the window's three sections, a click selects the one
// under the pointer, and Ctrl+2 reaches the second from anywhere.
TestCase {
    id: testCase
    name: "Strip"
    width: 600
    height: 200
    visible: true
    when: windowShown

    Component {
        id: stripComponent

        Item {
            property alias strip: navStrip
            property int chosen: -1

            NavStrip {
                id: navStrip
                model: Sections.all
                onActivated: function(index) { parent.chosen = index }
            }

            SectionShortcuts {
                onActivated: function(index) { parent.chosen = index }
            }
        }
    }

    function test_the_window_has_four_sections() {
        compare(Sections.all.length, 4)
        compare(Sections.all.map(function(s) { return s.id }),
                ["network", "bluetooth", "audio", "appearance"])
    }

    function test_clicking_the_second_item_selects_it() {
        const host = createTemporaryObject(stripComponent, testCase)
        verify(host)
        // Each item is a NavItem; walk the strip's children for them.
        const items = []
        const walk = function(node) {
            for (let i = 0; i < node.children.length; ++i) {
                const child = node.children[i]
                if (child.hasOwnProperty("iconName"))
                    items.push(child)
                walk(child)
            }
        }
        walk(host.strip)
        compare(items.length, 4)
        mouseClick(items[1])
        compare(host.chosen, 1)
    }

    function test_ctrl_2_selects_the_second_section() {
        const host = createTemporaryObject(stripComponent, testCase)
        verify(host)
        keyClick(Qt.Key_2, Qt.ControlModifier)
        compare(host.chosen, 1)
    }
}
