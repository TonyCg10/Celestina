import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

// The shared button's three shared states: a press sinks the fill and the
// content by the theme's recoil, a checked toggle paints as Selected, and an
// icon button's fill is a circle whatever density it has.
TestCase {
    id: testCase

    name: "CelestinaButton"
    when: testWindow.visible

    Window {
        id: testWindow

        width: 320
        height: 220
        visible: true

        CelestinaButton {
            id: plain
            x: 20
            y: 20
            text: "Guardar"
        }

        CelestinaButton {
            id: toggle
            x: 20
            y: 80
            text: "Ocultos"
            checkable: true
        }

        CelestinaButton {
            id: ghost
            x: 20
            y: 140
            text: "Volver"
            role: CelestinaButton.Ghost
        }

        CelestinaIconButton {
            id: glyph
            x: 200
            y: 20
            iconName: "x"
            helpText: "Cerrar"
        }

        CelestinaIconButton {
            id: bigGlyph
            x: 200
            y: 80
            density: CelestinaButton.Regular
            iconName: "x"
            helpText: "Cerrar"
        }
    }

    function init() {
        CelestinaTheme.reducedMotion = true
        toggle.checked = false
    }

    function cleanup() {
        CelestinaTheme.reducedMotion = false
    }

    function test_a_press_sinks_fill_and_content() {
        compare(plain.background.scale, 1)
        mousePress(plain, plain.width / 2, plain.height / 2)
        tryCompare(plain, "down", true)
        fuzzyCompare(plain.background.scale, CelestinaTheme.pressRecoilScale, 0.001)
        fuzzyCompare(plain.contentItem.scale, CelestinaTheme.pressRecoilScale, 0.001)
        mouseRelease(plain, plain.width / 2, plain.height / 2)
        tryCompare(plain, "down", false)
        fuzzyCompare(plain.background.scale, 1, 0.001)
    }

    function test_the_hit_box_does_not_shrink_with_the_recoil() {
        mousePress(plain, plain.width / 2, plain.height / 2)
        tryCompare(plain, "down", true)
        compare(plain.scale, 1)
        mouseRelease(plain, plain.width / 2, plain.height / 2)
    }

    function test_a_checked_toggle_is_selected() {
        compare(toggle.effectiveRole, CelestinaButton.Tonal)
        mouseClick(toggle, toggle.width / 2, toggle.height / 2)
        tryCompare(toggle, "checked", true)
        compare(toggle.effectiveRole, CelestinaButton.Selected)
        mouseClick(toggle, toggle.width / 2, toggle.height / 2)
        tryCompare(toggle, "checked", false)
        compare(toggle.effectiveRole, CelestinaButton.Tonal)
    }

    // The neutral ladder: rest, hover and press are three visible steps, and
    // the same two tokens lift every neutral control whatever its role.
    function test_a_tonal_button_lifts_from_its_plate_under_the_pointer() {
        mouseMove(testWindow, 300, 200)
        tryCompare(plain, "hovered", false)
        verify(Qt.colorEqual(plain.background.color, CelestinaTheme.controlFill))
        mouseMove(plain, plain.width / 2, plain.height / 2)
        tryCompare(plain, "hovered", true)
        tryVerify(function() { return Qt.colorEqual(plain.background.color, CelestinaTheme.controlHover) })
        mousePress(plain, plain.width / 2, plain.height / 2)
        tryCompare(plain, "down", true)
        tryVerify(function() { return Qt.colorEqual(plain.background.color, CelestinaTheme.pressedWash) })
        mouseRelease(plain, plain.width / 2, plain.height / 2)
        mouseMove(testWindow, 300, 200)
    }

    function test_a_ghost_button_lifts_from_clear_under_the_pointer() {
        mouseMove(testWindow, 300, 200)
        tryCompare(ghost, "hovered", false)
        verify(Qt.colorEqual(ghost.background.color, CelestinaTheme.clear))
        mouseMove(ghost, ghost.width / 2, ghost.height / 2)
        tryCompare(ghost, "hovered", true)
        tryVerify(function() { return Qt.colorEqual(ghost.background.color, CelestinaTheme.surfaceHover) })
        mousePress(ghost, ghost.width / 2, ghost.height / 2)
        tryCompare(ghost, "down", true)
        tryVerify(function() { return Qt.colorEqual(ghost.background.color, CelestinaTheme.pressedWash) })
        mouseRelease(ghost, ghost.width / 2, ghost.height / 2)
        mouseMove(testWindow, 300, 200)
    }

    function test_an_icon_button_is_a_circle_at_every_density() {
        compare(glyph.width, glyph.height)
        fuzzyCompare(glyph.backgroundRadius, glyph.height / 2, 0.001)
        fuzzyCompare(glyph.background.radius, glyph.height / 2, 0.001)
        fuzzyCompare(bigGlyph.background.radius, bigGlyph.height / 2, 0.001)
    }
}
