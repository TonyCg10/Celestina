import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// A change of the appearance file, queued by the adapter, reaches the theme
// through the same wiring as `Main.qml`: the text scale grows the type roles
// and reduced motion turns on.
TestCase {
    id: testCase
    name: "Appearance"

    QtObject {
        id: adapterStub
        property bool appearanceReducedMotion: false
        property real appearanceTextScale: 1.0
    }

    CelestinaAppearance {
        reducedMotion: adapterStub.appearanceReducedMotion
        textScale: adapterStub.appearanceTextScale
    }

    function cleanup() {
        adapterStub.appearanceReducedMotion = false
        adapterStub.appearanceTextScale = 1.0
    }

    function test_a_queued_change_reaches_the_theme() {
        compare(CelestinaTheme.fontBody, 13)
        adapterStub.appearanceTextScale = 1.3
        adapterStub.appearanceReducedMotion = true
        compare(CelestinaTheme.textScale, 1.3)
        compare(CelestinaTheme.fontBody, Math.round(13 * 1.3))
        verify(CelestinaTheme.reducedMotion)
        compare(CelestinaTheme.rowHeight, 40)
    }
}
