import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

// The shared text field mirrors Qt Controls' `visualFocus`, which a TextField
// template does not expose: the exterior ring and the lifted focus fill answer
// Tab, Backtab and a shortcut, and never a pointer or any other focus reason.
// Beyond that it must stay an ordinary field — reachable and editable from the
// keyboard, named by its consumer, and shaped by a closed role.
//
// Qt reports a TextField as editable text only while an assistive technology
// is active (`QAccessibleQuickItem::role()` falls back to EditableText for a
// `QQuickTextInput`). No assistive technology runs here, so the name is the
// accessibility fact these cases can observe; what an AT-SPI client hears is
// author validation.
TestCase {
    id: testCase

    name: "CelestinaTextField"
    when: testWindow.visible

    Window {
        id: testWindow

        width: 360
        height: 200
        visible: true

        Item {
            id: before
            width: 10
            height: 10
            activeFocusOnTab: true
        }

        CelestinaTextField {
            id: field
            x: 20
            y: 30
            width: 200
            placeholderText: "Name"
            Accessible.name: "File name"
        }

        Item {
            id: after
            x: 300
            width: 10
            height: 10
            activeFocusOnTab: true
        }

        CelestinaTextField {
            id: search
            x: 20
            y: 110
            width: 200
            shape: CelestinaTextField.Search
            activeFocusOnTab: false
        }
    }

    function ring(control) {
        return findChild(control, "textFieldFocusRing")
    }

    function assertKeyboardFocusVisible() {
        compare(field.visualFocus, true)
        verify(ring(field).visible, "keyboard focus shows no ring")
        verify(Qt.colorEqual(field.background.color, CelestinaTheme.inputFillFocus),
               "keyboard focus does not lift the fill")
    }

    function assertKeyboardFocusHidden() {
        compare(field.visualFocus, false)
        verify(!ring(field).visible, "the keyboard ring shows without keyboard focus")
        verify(Qt.colorEqual(field.background.color, CelestinaTheme.inputFill))
    }

    function init() {
        CelestinaTheme.reducedMotion = true
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        field.enabled = true
        field.text = ""
        before.forceActiveFocus(Qt.MouseFocusReason)
        tryCompare(before, "activeFocus", true)
    }

    function cleanup() {
        CelestinaTheme.reducedMotion = false
    }

    function test_it_is_named_by_its_consumer_and_reachable_by_tab() {
        compare(field.Accessible.name, "File name")
        verify(field.activeFocusOnTab, "the keyboard cannot reach the field")
        verify(ring(field), "the field lost its focus ring")
        assertKeyboardFocusHidden()
    }

    function test_tab_shows_the_ring_and_the_focus_fill() {
        keyClick(Qt.Key_Tab)
        tryCompare(field, "activeFocus", true)
        compare(field.focusReason, Qt.TabFocusReason)
        assertKeyboardFocusVisible()
        compare(ring(field).border.width, CelestinaTheme.borderFocus)
        verify(Qt.colorEqual(ring(field).border.color, CelestinaTheme.focusRing))
    }

    function test_backtab_shows_the_ring() {
        after.forceActiveFocus(Qt.MouseFocusReason)
        tryCompare(after, "activeFocus", true)
        keyClick(Qt.Key_Backtab)
        tryCompare(field, "activeFocus", true)
        compare(field.focusReason, Qt.BacktabFocusReason)
        assertKeyboardFocusVisible()
    }

    function test_a_shortcut_shows_the_ring() {
        field.forceActiveFocus(Qt.ShortcutFocusReason)
        tryCompare(field, "activeFocus", true)
        assertKeyboardFocusVisible()
    }

    function test_a_pointer_focuses_without_the_ring() {
        mouseClick(field, field.width / 2, field.height / 2, Qt.LeftButton)
        tryCompare(field, "activeFocus", true)
        compare(field.focusReason, Qt.MouseFocusReason)
        assertKeyboardFocusHidden()
    }

    function test_other_focus_reasons_show_no_ring() {
        field.forceActiveFocus(Qt.OtherFocusReason)
        tryCompare(field, "activeFocus", true)
        assertKeyboardFocusHidden()
    }

    function test_leaving_by_keyboard_drops_the_ring() {
        keyClick(Qt.Key_Tab)
        tryCompare(field, "activeFocus", true)
        assertKeyboardFocusVisible()
        after.forceActiveFocus(Qt.TabFocusReason)
        tryCompare(field, "activeFocus", false)
        assertKeyboardFocusHidden()
    }

    function test_the_keyboard_edits_it() {
        keyClick(Qt.Key_Tab)
        tryCompare(field, "activeFocus", true)
        keyClick("a")
        keyClick("b")
        compare(field.text, "ab")
        keyClick(Qt.Key_Backspace)
        compare(field.text, "a")
    }

    function test_the_shape_is_a_closed_role() {
        compare(field.shape, CelestinaTextField.Standard)
        compare(field.background.radius, CelestinaTheme.radiusSm)
        compare(search.background.radius, CelestinaTheme.radiusInput)
        compare(ring(search).cornerRadius, CelestinaTheme.radiusInput)
    }

    function test_a_disabled_field_is_skipped_by_tab() {
        field.enabled = false
        keyClick(Qt.Key_Tab)
        tryCompare(after, "activeFocus", true)
        verify(!field.activeFocus)
    }
}
