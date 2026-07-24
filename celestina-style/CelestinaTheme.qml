pragma Singleton

import QtQuick

// ─── CelestinaTheme ───────────────────────────────────────────────────────────
// Singleton design-token store for the entire Celestina suite: the single
// source of truth for the look. Organized as a One UI 8.5 (desktop-adapted)
// system — scales for radius, spacing, type, motion and glass, plus semantic
// colors. Tune these to art-direct the suite without touching component files.
// ──────────────────────────────────────────────────────────────────────────────
QtObject {
    // ══ PALETTE A — SAMSUNG ONE UI, neutral (ACTIVE) ═════════════════════
    // Dark near-black, neutral white accent (no lavender tint), native icon
    // colours; the glass identity is carried by the hairline outline. To swap
    // back to Rosé Pine: re-comment this block and un-comment PALETTE B below.
    readonly property color canvas: "#0B0C0E"
    readonly property color canvasRaised: "#16171A"
    readonly property color surface: "#D91E1F22"
    readonly property color surfaceStrong: "#F0202226"
    readonly property color surfaceHover: "#2A2E3036"
    readonly property color surfaceSelected: "#33FFFFFF"
    readonly property color border: "#2A8A93A0"
    readonly property color borderStrong: "#5AA9B4C4"
    readonly property color text: "#F4F6F8"
    readonly property color textMuted: "#A1A7B0"
    readonly property color accent: "#FFFFFF"
    readonly property color accentStrong: "#4C5157"
    readonly property color danger: "#FF6058"
    readonly property color focus: "#C9CFD8"
    readonly property color inputFill: "#4D101114"
    readonly property color inputFillFocus: "#66181B22"
    readonly property color inputBorder: "#245A616E"
    readonly property color controlFill: "#2B2A2C31"
    readonly property color badgeFill: "#28282A2F"
    readonly property color badgeAccentFill: "#22FFFFFF"
    readonly property color glyphDirectory: "#33343A42"
    readonly property color glyphSymlink: "#2C40444C"
    readonly property color glyphFile: "#2A2F3238"
    readonly property color dangerFill: "#4D5A2020"
    readonly property color dangerBorder: "#8AD16A63"
    readonly property color dangerText: "#FFD9D6"
    readonly property color gradientStart: "#0B0C0E"
    readonly property color gradientMid: "#111216"
    readonly property color gradientEnd: "#0C0D10"

    // ══ PALETTE B — ROSÉ PINE (PAUSED) ═══════════════════════════════════
    // Iris lavender accent, muted purple-grey surfaces, love = error.
    /*
    readonly property color canvas: "#191724"
    readonly property color canvasRaised: "#1F1D2E"
    readonly property color surface: "#D91F1D2E"
    readonly property color surfaceStrong: "#F026233A"
    readonly property color surfaceHover: "#2A403D52"
    readonly property color surfaceSelected: "#33C4A7E7"
    readonly property color border: "#2A524F67"
    readonly property color borderStrong: "#5A908CAA"
    readonly property color text: "#E0DEF4"
    readonly property color textMuted: "#908CAA"
    readonly property color accent: "#C4A7E7"
    readonly property color accentStrong: "#524F67"
    readonly property color danger: "#EB6F92"
    readonly property color focus: "#C4A7E7"

    // Semantic surfaces
    readonly property color inputFill: "#4D191724"
    readonly property color inputFillFocus: "#6626233A"
    readonly property color inputBorder: "#24403D52"
    readonly property color controlFill: "#2B26233A"
    readonly property color badgeFill: "#2826233A"
    readonly property color badgeAccentFill: "#26C4A7E7"

    // Entry-kind glyph tints
    readonly property color glyphDirectory: "#5A403D52"
    readonly property color glyphSymlink: "#4A403D52"
    readonly property color glyphFile: "#4A26233A"

    // Danger / error banner
    readonly property color dangerFill: "#3DEB6F92"
    readonly property color dangerBorder: "#80EB6F92"
    readonly property color dangerText: "#FBD7DE"

    // Canvas backdrop (subtle base gradient)
    readonly property color gradientStart: "#191724"
    readonly property color gradientMid: "#1E1B2B"
    readonly property color gradientEnd: "#16141F"
    */

    // ── Typography ───────────────────────────────────────────────────────
    readonly property string sansFamily: Qt.application.font.family
    readonly property string monoFamily: "monospace"

    readonly property int fontMini: 10
    readonly property int fontCaption: 11
    readonly property int fontLabel: 12
    readonly property int fontBody: 13
    readonly property int fontCallout: 15
    readonly property int fontTitle: 20
    readonly property int fontHeadline: 22
    readonly property int fontLargeTitle: 28
    readonly property int fontDisplay: 34

    readonly property int weightRegular: Font.Normal
    readonly property int weightMedium: Font.Medium
    readonly property int weightDemiBold: Font.DemiBold
    readonly property int weightBold: Font.Bold

    // ── Radius scale ─────────────────────────────────────────────────────
    // Pushed toward One UI's generous rounding. radiusPill makes any control a
    // full capsule — set a control's radius to radiusPill for a One UI pill.
    readonly property int radiusXs: 10   // badges, chips, menu items
    readonly property int radiusSm: 14   // controls, pills, glyph tiles, rows
    readonly property int radiusMd: 20   // glass menus / floating surfaces
    readonly property int radiusLg: 24   // panels / large rounded cards
    readonly property int radiusXl: 32
    readonly property int radiusPill: 9999

    // ── Spacing scale (4-based) ──────────────────────────────────────────
    readonly property int spaceXs: 4
    readonly property int spaceSm: 8
    readonly property int spaceMd: 12
    readonly property int spaceLg: 16
    readonly property int spaceXl: 20
    readonly property int space2xl: 24
    readonly property int space3xl: 32

    // ── Control metrics ──────────────────────────────────────────────────
    readonly property int controlHeight: 38
    readonly property int controlHeightLg: 42
    readonly property int rowHeight: 54
    readonly property int glyphTile: 34
    readonly property int iconSm: 18
    readonly property int iconMd: 19
    readonly property int menuWidth: 232
    readonly property int menuPadding: 6
    readonly property int menuMargins: 24

    // ── Motion ───────────────────────────────────────────────────────────
    readonly property int motionFast: 110
    readonly property int motionNormal: 170
    readonly property int motionSlow: 260

    // Easing vocabulary. easeEmphasized is the One UI "settle" (mild overshoot).
    readonly property int easeStandard: Easing.OutCubic
    readonly property int easeDecelerate: Easing.OutQuint
    readonly property int easeEmphasized: Easing.OutBack
    readonly property int easeExit: Easing.InCubic
    readonly property real overshoot: 1.15

    // ── Glass ────────────────────────────────────────────────────────────
    // Backdrop-blur parameters for GlassSurface.
    readonly property color glassTint: "#BD1E2028"       // Samsung One UI — frosted (mid)
    // readonly property color glassTint: "#A61F1D2E"    // Rosé Pine (paused)
    // One UI "glass with outline": a light hairline + a top-edge specular.
    readonly property color glassBorder: "#5CFFFFFF"
    readonly property color glassHighlight: "#2EFFFFFF"
    readonly property real glassBlur: 0.70
    readonly property int glassBlurMax: 30
    readonly property real glassSaturation: 0.14
    readonly property real glassSampleScale: 0.66
    readonly property int glassSampleMargin: 20

    // ── Icons ────────────────────────────────────────────────────────────
    // Minimal monochrome freedesktop-name fallbacks bundled with the module.
    readonly property string fallbackIconRoot:
        "qrc:/qt/qml/CelestinaStyle/icons/"

    function fallbackIcon(name) {
        return fallbackIconRoot + name + ".svg"
    }
}
