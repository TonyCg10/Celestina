pragma Singleton
import QtQuick

// ─── CelestinaTheme ───────────────────────────────────────────────────────────
// Singleton design token store for the entire Celestina suite.
// Based on Rosé Pine (Main) dark theme.
// Usage:  import CelestinaStyle 1.0  →  CelestinaTheme.base
// ──────────────────────────────────────────────────────────────────────────────

QtObject {

    // ── Rosé Pine Core Palette ────────────────────────────────────────────────
    readonly property color base:          "#191724"
    readonly property color surface:       "#1f1d2e"
    readonly property color overlay:       "#26233a"
    readonly property color muted:         "#6e6a86"
    readonly property color subtle:        "#908caa"
    readonly property color text:          "#e0def4"
    readonly property color love:          "#eb6f92"
    readonly property color gold:          "#f6c177"
    readonly property color rose:          "#ebbcba"
    readonly property color pine:          "#31748f"
    readonly property color foam:          "#9ccfd8"
    readonly property color iris:          "#c4a7e7"
    readonly property color highlightLow:  "#21202e"
    readonly property color highlightMed:  "#403d52"
    readonly property color highlightHigh: "#524f67"

    // ── Glassmorphic Variants ─────────────────────────────────────────────────
    readonly property color baseGlass:     Qt.rgba(0.098, 0.090, 0.141, 0.70)
    readonly property color surfaceGlass:  Qt.rgba(0.122, 0.114, 0.180, 0.60)
    readonly property color overlayGlass:  Qt.rgba(0.149, 0.137, 0.227, 0.50)

    // ── Typography ────────────────────────────────────────────────────────────
    readonly property string fontFamily:    "Inter"
    readonly property string monoFamily:    "JetBrains Mono"
    readonly property int    fontTiny:      10
    readonly property int    fontSmall:     12
    readonly property int    fontNormal:    13
    readonly property int    fontMedium:    15
    readonly property int    fontLarge:     18
    readonly property int    fontTitle:     22

    // ── Spacing & Radius ──────────────────────────────────────────────────────
    readonly property int    radiusSmall:   4
    readonly property int    radiusMedium:  8
    readonly property int    radiusLarge:   12
    readonly property int    radiusXL:      16

    readonly property int    spacingTiny:   4
    readonly property int    spacingSmall:  8
    readonly property int    spacingNormal: 12
    readonly property int    spacingLarge:  20

    // ── Borders ───────────────────────────────────────────────────────────────
    readonly property real   borderThin:    1.0
    readonly property real   borderMedium:  1.5
    readonly property real   borderThick:   2.0

    // ── Animation Durations ───────────────────────────────────────────────────
    readonly property int    animFast:      120
    readonly property int    animNormal:    200
    readonly property int    animSlow:      350

    // ── Glass Effect Parameters ───────────────────────────────────────────────
    readonly property real   glassBlur:     0.6
    readonly property real   glassSaturation: 0.15

    // ── Shadows ───────────────────────────────────────────────────────────────
    readonly property color  shadowColor:   Qt.rgba(0.0, 0.0, 0.0, 0.35)
    readonly property color  glowIris:      Qt.rgba(0.769, 0.655, 0.906, 0.15)
    readonly property color  glowPine:      Qt.rgba(0.192, 0.455, 0.561, 0.15)
    readonly property color  glowLove:      Qt.rgba(0.922, 0.435, 0.573, 0.15)
}
