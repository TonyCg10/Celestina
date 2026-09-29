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
        item.showSwatch = false
        item.automaticSwatch = false
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

    // The swatch's whole anatomy comes from the theme: its size, the slash an
    // automatic swatch draws and the outline of a chosen colour. A literal
    // here is invisible to the style guard, which is how it drifted before.
    function test_the_swatch_anatomy_comes_from_tokens() {
        item.showSwatch = true
        item.swatchColor = CelestinaTheme.accent
        wait(0)
        const swatch = findChild(item, "glassMenuSwatch")
        verify(swatch, "no swatch was painted")
        compare(swatch.width, CelestinaTheme.compMenuSwatchSize)
        compare(swatch.height, CelestinaTheme.compMenuSwatchSize)
        verify(Qt.colorEqual(swatch.border.color, CelestinaTheme.swatchOutline),
               "a chosen colour is not outlined by the swatch token")

        item.automaticSwatch = true
        wait(0)
        verify(Qt.colorEqual(swatch.border.color, CelestinaTheme.textMuted))
        const slash = swatch.children[0]
        verify(slash.visible, "an automatic swatch draws no slash")
        compare(slash.width, CelestinaTheme.compMenuSwatchSlash)
    }
}
