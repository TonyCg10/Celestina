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

        // A field of the content, and a bare pill floating over it: nothing
        // of the pill takes the press, so the field below does — and the
        // shield then takes the drag from it, which must not select there.
        TextField {
            id: coveredField
            x: 10
            y: 120
            width: 160
            text: "content under the pill"
        }

        Item {
            id: barePill
            x: 10
            y: 120
            width: 160
            height: 40

            CelestinaInputShield { swallowClicks: false }
        }

        // A pill that yields the drag to its own press: the shape every
        // floating button in Siderita has.
        Button {
            id: drifter
            x: 200
            y: 120
            width: 100
            height: 40
            text: "pill"
            hoverEnabled: true

            property int clicks: 0
            onClicked: clicks += 1

            CelestinaInputShield {
                swallowClicks: false
                yieldsToHost: true
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

    // The shield carries the selection of a field that is its surface's own;
    // one of the content it covers is exactly what it keeps from reacting.
    function test_a_covered_field_does_not_select_through_the_shield() {
        coveredField.select(0, 0)
        const y = barePill.height / 2
        mousePress(barePill, 4, y)
        for (let x = 6; x <= 120; x += 2)
            mouseMove(barePill, x, y)
        mouseRelease(barePill, 120, y)
        compare(coveredField.selectedText, "")
    }

    // The drag handler on the shield claims the press with a zero threshold.
    // Under a host that owns its own press it must never take that press
    // away: a click whose pointer drifts a pixel between press and release is
    // still a click, and the pill under it used to lose exactly those.
    function test_a_drifting_click_still_reaches_a_yielding_host() {
        drifter.clicks = 0
        const x = drifter.width / 2
        const y = drifter.height / 2
        mousePress(drifter, x, y)
        mouseMove(drifter, x + 3, y + 2)
        mouseMove(drifter, x + 5, y + 3)
        mouseRelease(drifter, x + 5, y + 3)
        tryCompare(drifter, "clicks", 1)
    }
}
