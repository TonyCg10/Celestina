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

    // Reading a colour property hands JavaScript a live reference to it, not a
    // snapshot: a `const` taken before the state changes reads back the *new*
    // colour afterwards, and every comparison against it trivially succeeds.
    // Sampling through `String()` freezes the value, which is the only reason
    // these tests can tell two fills apart at all.
    function sample(value) {
        return String(value)
    }

    function test_current_and_highlighted_differ() {
        const resting = sample(item.background.color)

        item.current = true
        wait(0)
        const current = sample(item.background.color)
        verify(!Qt.colorEqual(current, resting), "current paints nothing")

        item.current = false
        item.highlighted = true
        wait(0)
        const highlighted = sample(item.background.color)
        verify(!Qt.colorEqual(highlighted, resting), "highlight paints nothing")
        verify(!Qt.colorEqual(highlighted, current),
               "the cursor and the current value wear the same fill")
    }

    function test_the_cursor_wins_over_the_current_row() {
        item.highlighted = true
        wait(0)
        const highlighted = sample(item.background.color)
        item.current = true
        wait(0)
        verify(Qt.colorEqual(item.background.color, highlighted),
               "the current row overrode the cursor's fill")
    }
}
