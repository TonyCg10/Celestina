import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

// `current` and `highlighted` are two different statements — this is the
// value, and this is where the cursor is — and a choice menu makes both at
// once. They must read as two fills, and the cursor must stay visible while
// it passes over the current row.
TestCase {
    id: testCase

    name: "GlassMenuItem"
    when: testWindow.visible

    Window {
        id: testWindow

        width: 320
        height: 160
        visible: true

        GlassMenuItem {
            id: item
            text: "row"
        }
    }

    function init() {
        CelestinaTheme.reducedMotion = true
        item.current = false
        item.highlighted = false
        wait(0)
    }

    function test_current_and_highlighted_differ() {
        const resting = item.background.color

        item.current = true
        wait(0)
        const current = item.background.color
        verify(!Qt.colorEqual(current, resting), "current paints nothing")

        item.current = false
        item.highlighted = true
        wait(0)
        const highlighted = item.background.color
        verify(!Qt.colorEqual(highlighted, resting), "highlight paints nothing")
        verify(!Qt.colorEqual(highlighted, current),
               "the cursor and the current value wear the same fill")
    }

    function test_the_cursor_wins_over_the_current_row() {
        item.highlighted = true
        wait(0)
        const highlighted = item.background.color
        item.current = true
        wait(0)
        compare(item.background.color, highlighted)
    }
}
