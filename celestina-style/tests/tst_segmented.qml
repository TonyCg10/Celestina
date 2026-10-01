import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

// A row of peer destinations. What is asserted: the plate is radiusButton
// tall compSegmentHeight; the selected segment's plate is concentric with it;
// one Tab stop lands on the current segment; Left/Right/Home/End emit
// `activated` and the control never changes `currentIndex` itself; the roles
// are a page tab list of page tabs; under reduced motion the fill lands at once.
TestCase {
    id: testCase

    name: "CelestinaSegmentedControl"
    when: testWindow.visible

    property var activations: []

    Window {
        id: testWindow
        width: 480
        height: 160
        visible: true

        Item { id: before; width: 10; height: 10; activeFocusOnTab: true }

        CelestinaSegmentedControl {
            id: control
            x: 20
            y: 40
            helpText: "Secciones"
            model: [
                { key: "performance", icon: "gauge", label: "Rendimiento" },
                { key: "processes", icon: "view-list", label: "Procesos" },
                { key: "sensors", icon: "cpu", label: "Sensores" }
            ]
            currentIndex: 1
            onActivated: function(index) { testCase.activations.push(index) }
        }

        Item { id: after; x: 400; width: 10; height: 10; activeFocusOnTab: true }
    }

    function init() {
        testCase.activations = []
        control.currentIndex = 1
        control.iconOnly = false
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        before.forceActiveFocus(Qt.MouseFocusReason)
        tryCompare(before, "activeFocus", true)
    }

    function echo(index) {
        control.currentIndex = index
    }

    function segment(index) {
        return findChild(control, "segment-" + index)
    }

    function test_geometry_is_the_bar_slot() {
        compare(control.implicitHeight, CelestinaTheme.compSegmentHeight)
        const plate = findChild(control, "segmentedPlate")
        verify(plate)
        compare(plate.radius, CelestinaTheme.radiusButton)
        const selected = findChild(segment(1), "segmentPlate")
        verify(selected)
        compare(selected.radius, CelestinaTheme.radiusButton - CelestinaTheme.spaceXs)
        compare(segment(1).x, CelestinaTheme.spaceXs + segment(0).width + CelestinaTheme.spaceXs)
    }

    function test_one_tab_stop_lands_on_the_current_segment() {
        keyClick(Qt.Key_Tab)
        tryCompare(segment(1), "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(after, "activeFocus", true)
    }

    function test_arrows_emit_and_never_move_the_index_themselves() {
        before.forceActiveFocus(Qt.MouseFocusReason)
        tryCompare(before, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(segment(1), "activeFocus", true)
        keyClick(Qt.Key_Right)
        compare(testCase.activations, [2])
        compare(control.currentIndex, 1)
        keyClick(Qt.Key_Left)
        compare(testCase.activations, [2, 0])
        compare(control.currentIndex, 1)
    }

    function test_an_echoing_host_walks_the_segments() {
        control.onActivated.connect(testCase.echo)
        before.forceActiveFocus(Qt.MouseFocusReason)
        tryCompare(before, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(segment(1), "activeFocus", true)
        keyClick(Qt.Key_Right)
        tryCompare(segment(2), "activeFocus", true)
        keyClick(Qt.Key_Left)
        keyClick(Qt.Key_Left)
        keyClick(Qt.Key_Home)
        keyClick(Qt.Key_End)
        compare(testCase.activations, [2, 1, 0, 0, 2])
        compare(control.currentIndex, 2)
        control.onActivated.disconnect(testCase.echo)
    }

    function test_click_activates() {
        mouseClick(segment(2))
        compare(testCase.activations, [2])
    }

    function test_roles_and_names() {
        compare(control.Accessible.role, Accessible.PageTabList)
        compare(control.Accessible.name, "Secciones")
        compare(segment(0).Accessible.role, Accessible.PageTab)
        compare(segment(0).Accessible.name, "Rendimiento")
        compare(segment(1).Accessible.checked, true)
        compare(segment(0).Accessible.checked, false)
        compare(segment(1).Accessible.selected, true)
        compare(segment(0).Accessible.selected, false)
    }

    function test_icon_only_hides_the_labels_and_keeps_the_names() {
        control.iconOnly = true
        const label = findChild(segment(0), "segmentLabel")
        verify(label)
        verify(!label.visible)
        compare(segment(0).Accessible.name, "Rendimiento")
        verify(segment(0).width < 2 * CelestinaTheme.compSegmentHeight + CelestinaTheme.spaceMd)
    }

    function test_reduced_motion_lands_the_fill_at_once() {
        const plate = findChild(segment(0), "segmentPlate")
        verify(plate)
        compare(findChild(plate, "segmentFill").duration,
                CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionFast)
    }
}
