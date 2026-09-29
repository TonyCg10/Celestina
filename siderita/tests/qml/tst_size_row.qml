import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// STY-4: the sizes menu row is the system slider, not a restyled copy. It
// carries the row's label as its accessible name, moves one tenth per arrow
// key, and reports tenths — never the raw fraction a drag produces.
TestCase {
    id: testCase
    name: "SizeRow"
    width: 300
    height: 60
    visible: true
    when: windowShown

    property var reported: []

    SizeRow {
        id: row
        label: "Content"
        value: 1.0
        minValue: 0.75
        maxValue: 1.5
        onMoved: function(v) { testCase.reported.push(v) }
    }

    function init() {
        testCase.reported = []
        row.maxValue = 1.5
        row.value = 1.0
    }

    function slider() {
        return findChild(row, "sizeSlider")
    }

    function test_a_the_slider_is_named_by_the_row() {
        const control = testCase.slider()
        verify(control !== null)
        compare(control.Accessible.role, Accessible.Slider)
        compare(control.Accessible.name, "Content")
    }

    function test_b_an_arrow_key_moves_one_tenth() {
        const control = testCase.slider()
        control.forceActiveFocus()
        keyClick(Qt.Key_Right)
        compare(testCase.reported.length, 1)
        fuzzyCompare(testCase.reported[0], 1.1, 0.0001)
        keyClick(Qt.Key_Left)
        fuzzyCompare(testCase.reported[1], 0.9, 0.0001)
    }

    function test_c_the_value_is_bounded_by_the_row() {
        row.value = 1.5
        const control = testCase.slider()
        control.forceActiveFocus()
        keyClick(Qt.Key_Right)
        fuzzyCompare(testCase.reported[0], 1.5, 0.0001)
    }

    // A bound that is not a whole tenth is reported as it is: rounding it
    // would step outside the row (1.25 → 1.3) or never reach it (0.75 → 0.8).
    function test_d_a_bound_off_the_tenths_is_reported_exactly() {
        row.maxValue = 1.25
        row.value = 1.2
        const control = testCase.slider()
        control.forceActiveFocus()
        keyClick(Qt.Key_Right)
        fuzzyCompare(testCase.reported[0], 1.25, 0.0001)
        fuzzyCompare(row.snap(1.24), 1.2, 0.0001)
        compare(row.snap(2.0), 1.25)

        row.value = 0.8
        keyClick(Qt.Key_Left)
        fuzzyCompare(testCase.reported[1], 0.75, 0.0001)
        compare(row.snap(0.1), 0.75)
    }
}
