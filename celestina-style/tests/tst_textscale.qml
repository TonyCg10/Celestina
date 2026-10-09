import QtQuick
import QtTest
import CelestinaStyle

// The text scale multiplies the nine type roles and nothing else: at the
// largest step `fontBody` is the rounded product of its base, and the layout
// tokens a row is built from keep their value.
TestCase {
    name: "CelestinaThemeTextScale"

    QtObject {
        id: adapterStub
        property bool reducedMotion: false
        property real textScale: 1.0
    }

    Component {
        id: appearanceComponent
        CelestinaAppearance {
            reducedMotion: adapterStub.reducedMotion
            textScale: adapterStub.textScale
        }
    }

    function cleanup() {
        adapterStub.reducedMotion = false
        adapterStub.textScale = 1.0
        CelestinaTheme.textScale = 1.0
        CelestinaTheme.reducedMotion = false
    }

    function test_the_appearance_binding_feeds_the_theme() {
        const binding = createTemporaryObject(appearanceComponent, null)
        verify(binding !== null)
        adapterStub.textScale = 1.15
        adapterStub.reducedMotion = true
        compare(CelestinaTheme.textScale, 1.15)
        compare(CelestinaTheme.fontBody, Math.round(13 * 1.15))
        compare(CelestinaTheme.reducedMotion, true)
    }

    function test_default_scale_keeps_the_bases() {
        compare(CelestinaTheme.textScale, 1.0)
        compare(CelestinaTheme.fontBody, 13)
        compare(CelestinaTheme.fontMini, 10)
        compare(CelestinaTheme.fontDisplay, 34)
    }

    function test_largest_scale_rounds_every_role() {
        CelestinaTheme.textScale = 1.3
        compare(CelestinaTheme.fontBody, Math.round(13 * 1.3))
        compare(CelestinaTheme.fontMini, Math.round(10 * 1.3))
        compare(CelestinaTheme.fontCaption, Math.round(11 * 1.3))
        compare(CelestinaTheme.fontRowSecondary, Math.round(12 * 1.3))
        compare(CelestinaTheme.fontRowTitle, Math.round(15 * 1.3))
        compare(CelestinaTheme.fontTitle, Math.round(17 * 1.3))
        compare(CelestinaTheme.fontHeaderCollapsed, Math.round(20 * 1.3))
        compare(CelestinaTheme.fontHeaderExpanded, Math.round(30 * 1.3))
        compare(CelestinaTheme.fontDisplay, Math.round(34 * 1.3))
    }

    function test_layout_tokens_do_not_scale() {
        const row = CelestinaTheme.rowHeight
        const radius = CelestinaTheme.radiusSm
        CelestinaTheme.textScale = 1.3
        compare(CelestinaTheme.rowHeight, row)
        compare(CelestinaTheme.radiusSm, radius)
    }
}
