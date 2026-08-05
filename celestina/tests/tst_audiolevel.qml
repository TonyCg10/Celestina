import QtQuick
import QtTest
import "../qml" as Desktop

// language-contract: allow-non-english
//
// The assertions carry the shipped Spanish product copy, because what this
// checks is the exact text a screen reader is handed. Everything else in the
// file — names, comments, structure — is English development truth.
//
// The absent reading, which is where the live session threw. The widget hides
// itself when the provider goes away, but an `Accessible` binding is evaluated
// regardless, and reaching into an undefined reading there produced a TypeError
// on every frame the helper missed.
TestCase {
    id: testCase

    name: "AudioLevel"

    Desktop.AudioLevel {
        id: level

        reading: ({"volume": 40, "muted": false, "micVolume": 70, "micMuted": false})
    }

    // Each case starts from a device that is there and audible; the cases run
    // in name order and would otherwise inherit each other's readings.
    function init() {
        level.reading = ({"volume": 40, "muted": false, "micVolume": 70, "micMuted": false});
    }

    function test_a_reading_is_shown_as_whole_percent() {
        verify(level.hasReading);
        compare(level.spokenVolume, "Volumen 40 %");
    }

    function test_a_muted_device_says_so_and_keeps_its_level() {
        level.reading = ({"volume": 40, "muted": true});
        compare(level.spokenVolume, "Volumen silenciado, 40 %");
    }

    function test_an_absent_reading_never_reaches_into_it() {
        level.reading = undefined;

        verify(!level.hasReading);
        // The binding still runs; it must not touch `reading.volume`.
        compare(level.spokenVolume, "Volumen sin lectura");
        compare(level.implicitWidth, 0);
        // The microphone half is guarded by the same rule.
        compare(level.spokenMic, "Micrófono sin lectura");
    }

    function test_a_provider_that_comes_back_is_shown_again() {
        level.reading = undefined;
        level.reading = ({"volume": 55, "muted": false});

        verify(level.hasReading);
        compare(level.spokenVolume, "Volumen 55 %");
    }
}
