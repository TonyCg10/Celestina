import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

// The shared switch is a real Qt `Switch` whose indicator the module restyles.
// What is asserted here is what the restyle must not lose: it stays a
// checkable button that Space toggles, it is named by its consumer, the
// exterior focus ring answers keyboard focus and never a pointer, the track
// and thumb carry the state, and under reduced motion the thumb lands at once.
//
// Qt derives the accessible role and checked state only while an assistive
// technology is active: `QQuickAbstractButton::accessibleRole()` answers
// CheckBox for a checkable button, and `QAccessibleQuickItem::state()` reads
// `checked`. No assistive technology runs here, so these cases assert the
// inputs Qt derives them from; what an AT-SPI client hears is author
// validation.
TestCase {
    id: testCase

    name: "CelestinaSwitch"
    when: testWindow.visible

    property int toggles: 0

    Window {
        id: testWindow

        width: 320
        height: 160
        visible: true

        Item {
            id: before
            width: 10
            height: 10
            activeFocusOnTab: true
        }

        CelestinaSwitch {
            id: control
            x: 20
            y: 60
            Accessible.name: "Night light"
            onToggled: testCase.toggles += 1
        }

        Item {
            id: after
            x: 200
            width: 10
            height: 10
            activeFocusOnTab: true
        }
    }

    // The ring is the one child of the track that frames it.
    function ring() {
        const track = control.indicator
        for (let i = 0; i < track.children.length; ++i) {
            if (track.children[i].target === track)
                return track.children[i]
        }
        return null
    }

    function thumb() {
        const track = control.indicator
        for (let i = 0; i < track.children.length; ++i) {
            const child = track.children[i]
            if (child.target === undefined
                    && child.width === CelestinaTheme.compSwitchThumbSize)
                return child
        }
        return null
    }

    function thumbEnd() {
        return control.indicator.width - CelestinaTheme.compSwitchThumbSize
                - CelestinaTheme.compSwitchThumbInset
    }

    function init() {
        CelestinaTheme.reducedMotion = true
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        control.enabled = true
        control.checked = false
        toggles = 0
        before.forceActiveFocus(Qt.MouseFocusReason)
        tryCompare(before, "activeFocus", true)
    }

    function cleanup() {
        CelestinaTheme.reducedMotion = false
    }

    function test_it_is_a_checkable_button_named_by_its_consumer() {
        verify(control.checkable, "Qt announces only a checkable button as a check box")
        compare(control.text, "", "the switch paints no label; its row carries it")
        compare(control.Accessible.name, "Night light")
        verify(control.activeFocusOnTab, "the keyboard cannot reach the switch")
    }

    function test_tab_reaches_it_and_shows_the_focus_ring() {
        const focusRing = ring()
        verify(focusRing, "the switch lost its focus ring")
        verify(!focusRing.visible, "the ring shows without focus")

        keyClick(Qt.Key_Tab)
        tryCompare(control, "activeFocus", true)
        compare(control.visualFocus, true)
        verify(focusRing.visible, "keyboard focus shows no ring")
        compare(focusRing.border.width, CelestinaTheme.borderFocus)
        verify(Qt.colorEqual(focusRing.border.color, CelestinaTheme.focusRing))
    }

    function test_backtab_shows_the_focus_ring_too() {
        after.forceActiveFocus(Qt.MouseFocusReason)
        tryCompare(after, "activeFocus", true)
        keyClick(Qt.Key_Backtab)
        tryCompare(control, "activeFocus", true)
        compare(control.visualFocus, true)
        verify(ring().visible)
    }

    function test_space_toggles_it() {
        keyClick(Qt.Key_Tab)
        tryCompare(control, "activeFocus", true)

        keyClick(Qt.Key_Space)
        compare(control.checked, true)
        compare(toggles, 1)

        keyClick(Qt.Key_Space)
        compare(control.checked, false)
        compare(toggles, 2)
    }

    function test_a_pointer_toggles_it_without_the_ring() {
        mouseClick(control, control.width / 2, control.height / 2, Qt.LeftButton)
        compare(control.checked, true)
        compare(toggles, 1)
        compare(control.visualFocus, false)
        verify(!ring().visible, "a pointer click painted the keyboard ring")
    }

    function test_the_track_and_thumb_carry_the_state() {
        const knob = thumb()
        verify(knob, "the switch lost its thumb")
        verify(Qt.colorEqual(control.indicator.color, CelestinaTheme.controlFill))
        compare(knob.x, CelestinaTheme.compSwitchThumbInset)

        control.toggle()
        verify(Qt.colorEqual(control.indicator.color, CelestinaTheme.accent))
        compare(knob.x, thumbEnd())
        compare(control.indicator.width, CelestinaTheme.compSwitchTrackWidth)
        compare(control.indicator.height, CelestinaTheme.compSwitchTrackHeight)
    }

    // A zero-length animation completes when it starts, so under reduced
    // motion the thumb is already home on the line after the toggle. Without
    // reduced motion the same read finds it still at its start.
    function test_reduced_motion_lands_the_thumb_at_once() {
        const knob = thumb()
        control.toggle()
        compare(knob.x, thumbEnd())
    }

    function test_without_reduced_motion_the_thumb_travels() {
        CelestinaTheme.reducedMotion = false
        const knob = thumb()
        control.toggle()
        verify(knob.x < thumbEnd(), "the thumb jumped although motion is allowed")
        tryCompare(knob, "x", thumbEnd())
    }

    function test_a_disabled_switch_takes_neither_tab_nor_pointer() {
        control.enabled = false
        keyClick(Qt.Key_Tab)
        tryCompare(after, "activeFocus", true)
        verify(!control.activeFocus)

        mouseClick(control, control.width / 2, control.height / 2, Qt.LeftButton)
        compare(control.checked, false)
        compare(toggles, 0)
        compare(control.indicator.opacity, CelestinaTheme.disabledOpacity)
    }
}
