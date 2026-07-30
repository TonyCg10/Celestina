import QtQuick
import QtTest
import "../qml" as Desktop

TestCase {
    id: testCase

    name: "OutputChooser"

    function screen(name, width) {
        return {
            "name": name,
            "width": width,
            "height": 1080,
            "devicePixelRatio": 1
        }
    }

    Desktop.OutputChooser {
        id: chooser

        visible: false
        reducedMotion: true
        screens: []
    }

    function init() {
        chooser.chosen = ""
        chooser.cancelled = false
        chooser.screens = [screen("DP-1", 1920),
                           screen("DP-2", 2560),
                           screen("HDMI-A-1", 1920)]
        chooser.selectOutput(1)
        compare(chooser.selectedOutputName, "DP-2")
    }

    function test_reorder_preserves_output_identity() {
        chooser.screens = [screen("HDMI-A-1", 1920),
                           screen("DP-2", 2560),
                           screen("DP-1", 1920)]

        compare(chooser.selected, 1)
        compare(chooser.selectedOutputName, "DP-2")
    }

    function test_removing_an_earlier_output_preserves_selection() {
        chooser.screens = [screen("DP-2", 2560),
                           screen("HDMI-A-1", 1920)]

        compare(chooser.selected, 0)
        compare(chooser.selectedOutputName, "DP-2")
    }

    function test_removing_selected_output_uses_bounded_fallback() {
        chooser.screens = [screen("DP-1", 1920),
                           screen("HDMI-A-1", 1920)]

        compare(chooser.selected, 1)
        compare(chooser.selectedOutputName, "HDMI-A-1")
    }
}
