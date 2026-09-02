import QtQuick
import QtQuick.Controls
import QtQuick.Window
import QtTest
import CelestinaStyle

// The shield's hover block, seen from a host that has hover of its own. Qt
// delivers hover leaf-first, so a blocking handler on a child also stops the
// parent: a Button shielding what it floats over never reported `hovered`.
// `blockHover: false` hands the hover back; the shield's own `hovered` is the
// other way out for a host that wants to keep the block.
TestCase {
    id: testCase

    name: "CelestinaInputShield"
    when: testWindow.visible

    Window {
        id: testWindow

        width: 320
        height: 200
        visible: true

        // What the button floats over: a row that lights up under a cursor.
        MouseArea {
            id: below
            anchors.fill: parent
            hoverEnabled: true
        }

        Button {
            id: host
            x: 60
            y: 60
            width: 120
            height: 40
            text: "float"
            hoverEnabled: true

            CelestinaInputShield {
                id: shield
                swallowClicks: false
            }
        }
    }

    function init() {
        shield.blockHover = true
        mouseMove(testWindow, 5, 190)
        wait(0)
        tryCompare(host, "hovered", false)
    }

    function test_the_block_takes_the_hosts_hover_too() {
        mouseMove(host, host.width / 2, host.height / 2)
        tryCompare(shield, "hovered", true)
        compare(host.hovered, false)
        compare(below.containsMouse, false)
    }

    function test_block_hover_off_returns_the_hosts_hover() {
        shield.blockHover = false
        mouseMove(host, host.width / 2, host.height / 2)
        tryCompare(host, "hovered", true)
        compare(shield.hovered, true)
    }
}
