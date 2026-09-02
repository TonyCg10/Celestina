import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

// The slider's pointer anatomy: a thumb that sits where the value is, and a
// hover and a pressed state a pointer can see. Colours are compared as states
// against each other, not against literals — which token each state wears is
// the theme's business; that the states differ is the control's.
TestCase {
    id: testCase

    name: "CelestinaSlider"
    when: testWindow.visible

    property real lastMoved: -1

    Window {
        id: testWindow

        width: 320
        height: 120
        visible: true

        CelestinaSlider {
            id: slider

            x: 20
            y: 40
            width: 200
            value: 25
            to: 100
            step: 5
            onMoved: function(target) { testCase.lastMoved = target }
        }
    }

    function thumb() {
        return findChild(slider, "sliderThumb")
    }

    function init() {
        CelestinaTheme.reducedMotion = true
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        slider.enabled = true
        lastMoved = -1
        mouseMove(testWindow, 5, 110)
        wait(0)
    }

    function test_the_control_is_as_tall_as_the_compact_controls() {
        compare(slider.implicitHeight, CelestinaTheme.controlHeightXs)
    }

    function test_the_thumb_sits_at_the_value() {
        const handle = thumb()
        verify(handle, "no thumb was painted")
        compare(handle.width, CelestinaTheme.compSliderHandleSize)
        compare(handle.height, CelestinaTheme.compSliderHandleSize)
        // The thumb's centre in control coordinates lands on the value's
        // fraction of the inset track, and never outside the control's box.
        const centre = handle.parent.x + handle.x + handle.width / 2
        const trackStart = slider.handleSize / 2
        const trackWidth = slider.width - slider.handleSize
        fuzzyCompare(centre, trackStart + trackWidth * slider.fraction, 0.5)
        verify(handle.parent.x + handle.x >= 0)
    }

    function test_hover_and_press_are_distinct_states() {
        const handle = thumb()
        const resting = handle.color

        mouseMove(slider, slider.width / 2, slider.height / 2)
        tryCompare(slider, "hovered", true)
        const hovering = handle.color
        verify(!Qt.colorEqual(hovering, resting),
               "the thumb does not change under the pointer")

        mousePress(slider, slider.width / 2, slider.height / 2, Qt.LeftButton)
        tryCompare(slider, "dragging", true)
        const pressed = handle.color
        verify(!Qt.colorEqual(pressed, hovering),
               "the thumb does not change while dragged")
        verify(!Qt.colorEqual(pressed, resting))

        mouseRelease(slider, slider.width / 2, slider.height / 2, Qt.LeftButton)
        tryCompare(slider, "dragging", false)
        tryCompare(handle, "color", hovering)
    }

    // A press lands the thumb under the pointer: the same inset that keeps the
    // thumb inside the box is the one the pointer is mapped against.
    function test_a_press_maps_through_the_track_inset() {
        const trackStart = slider.handleSize / 2
        const trackWidth = slider.width - slider.handleSize
        mouseClick(slider, trackStart + trackWidth / 2, slider.height / 2,
                   Qt.LeftButton)
        fuzzyCompare(lastMoved, 50, 1)
    }

    function test_a_wheel_notch_is_one_step() {
        slider.wheelEnabled = true
        mouseWheel(slider, slider.width / 2, slider.height / 2, 0, 120,
                   Qt.NoButton, Qt.NoModifier, 0)
        tryCompare(testCase, "lastMoved", 30)
        slider.wheelEnabled = false
        mouseWheel(slider, slider.width / 2, slider.height / 2, 0, 120,
                   Qt.NoButton, Qt.NoModifier, 0)
        wait(0)
        compare(lastMoved, 30)
    }

    function test_a_disabled_slider_neither_hovers_nor_drags() {
        slider.enabled = false
        mouseMove(slider, slider.width / 2, slider.height / 2)
        wait(0)
        compare(slider.hovered, false)
        mouseClick(slider, slider.width / 2, slider.height / 2, Qt.LeftButton)
        compare(lastMoved, -1)
    }
}
