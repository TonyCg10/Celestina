import QtQuick
import QtTest
import "../qml" as Desktop

// Brightness is a stable panel affordance even on an output that has no DDC
// control. Only a real reading is allowed to produce a step request.
TestCase {
    id: testCase

    name: "BrightnessLevel"

    Desktop.BackdropInk {
        id: testInk
    }

    Desktop.BrightnessLevel {
        id: brightness

        ink: testInk
        outputName: "DP-1"
        reading: undefined
    }

    SignalSpy {
        id: steps

        target: brightness
        signalName: "stepRequested"
    }

    function init() {
        brightness.reading = undefined;
        steps.clear();
    }

    function test_the_sun_remains_without_a_ddc_reading() {
        verify(!brightness.offered);
        compare(findChild(brightness, "celestina-brightness-icon").name, "sun");
        verify(brightness.implicitWidth > 0);
    }

    function test_a_real_reading_stays_in_the_accessible_name() {
        brightness.reading = {"DP-1": 65};
        verify(brightness.offered);
        verify(brightness.known);
        verify(brightness.Accessible.name.indexOf("65") >= 0);
    }
}
