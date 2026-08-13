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
        // Now an opener like every other panel control, so it answers for the
        // bar's compositor glass the way its neighbours do.
        blurAvailable: false
    }

    SignalSpy {
        id: steps

        target: brightness
        signalName: "stepRequested"
    }

    SignalSpy {
        id: menus

        target: brightness
        signalName: "menuRequested"
    }

    function init() {
        brightness.reading = undefined;
        steps.clear();
        menus.clear();
    }

    function test_the_sun_remains_without_a_ddc_reading() {
        verify(!brightness.offered);
        compare(findChild(brightness, "celestina-brightness-button-icon").name,
                "sun");
        verify(brightness.implicitWidth > 0);
    }

    // The way in to every other monitor, and it must survive the case where
    // this output has nothing of its own: an output without DDC is exactly
    // when the author needs to be shown which outputs have it.
    function test_an_output_without_ddc_can_still_open_the_menu() {
        verify(!brightness.offered);
        verify(brightness.enabled);
        brightness.requestMenu();
        compare(menus.count, 1);
        // Both rectangles are real, so the body can be placed and the drop's
        // mouth can be aimed at the glyph rather than the whole control.
        const opener = menus.signalArguments[0][0];
        const anchor = menus.signalArguments[0][1];
        verify(opener.width > 0 && opener.height > 0);
        verify(anchor.width > 0 && anchor.height > 0);
    }

    function test_a_real_reading_stays_in_the_accessible_name() {
        brightness.reading = {"DP-1": 65};
        verify(brightness.offered);
        verify(brightness.known);
        verify(brightness.Accessible.name.indexOf("65") >= 0);
    }
}
