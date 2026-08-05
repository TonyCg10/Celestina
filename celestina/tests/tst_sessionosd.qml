import QtQuick
import QtTest
import "../qml" as Desktop

// What the on-screen display paints for the values it is handed. It is
// constructed offscreen here: this proves the component builds and reads
// truthfully, never how it looks on a compositor — that stays in VALIDATION.md.
TestCase {
    id: testCase

    name: "SessionOsd"

    Desktop.SessionOsd {
        id: osd

        kind: "volume"
        percent: 40
        muted: false
        label: ""
        reducedMotion: false
    }

    function test_a_level_reads_as_whole_percent() {
        osd.kind = "volume";
        osd.percent = 40;
        osd.muted = false;
        compare(osd.headline, "Volumen");
        compare(osd.valueText, "40 %");
        verify(osd.hasLevel);
    }

    function test_a_muted_device_says_so_instead_of_its_number() {
        osd.kind = "volume";
        osd.percent = 40;
        osd.muted = true;
        compare(osd.valueText, "Silenciado");
        // The level it remembers is still there to be drawn.
        verify(osd.hasLevel);
    }

    function test_no_reading_is_not_a_level_of_zero() {
        osd.kind = "microphone";
        osd.percent = -1;
        osd.muted = false;
        verify(!osd.hasLevel);
        compare(osd.valueText, "Sin lectura");
    }

    function test_a_monitor_is_named_in_its_title() {
        osd.kind = "brightness";
        osd.percent = 55;
        osd.muted = false;
        osd.label = "DP-2";
        compare(osd.headline, "Brillo — DP-2");
        compare(osd.valueText, "55 %");
    }

    function test_what_is_spoken_carries_both_facts() {
        osd.kind = "volume";
        osd.percent = 30;
        osd.muted = false;
        osd.label = "";
        compare(osd.spokenText, "Volumen: 30 %");
    }

    function test_an_unknown_kind_is_shown_rather_than_dropped() {
        osd.kind = "night-light";
        compare(osd.headline, "night-light");
    }
}
