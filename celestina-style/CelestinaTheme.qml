pragma Singleton

import QtQuick

// ─── CelestinaTheme ───────────────────────────────────────────────────────────
// Singleton design-token store for the whole Celestina suite: the single source
// of truth for the look, a One UI 8.5 system adapted to a pointer desktop.
//
// Three tiers (DESIGN.md §6.1):
//   ref.*    primitive ramps (SESL neutrals, the One UI accent). Apps NEVER read
//            ref.* — only schemes map a primitive onto a role. Grep for
//            `CelestinaTheme.ref` in an app is a bug.
//   scheme   semantic roles as DATA — a ColorScheme object, not comment-toggled
//            palette blocks. Every surface role ships its `on*` pair, so the
//            contrast contract is enforced by the type (a scheme that forgets a
//            role will not compile) instead of living as tribal knowledge.
//   sys.*    the flat properties apps consume (`CelestinaTheme.canvas`); each is
//            just a read of the active scheme, so swapping schemes later is one
//            property flip, not a migration.
//   comp*    per-component knobs, kept minimal (menu geometry today).
//
// Sealed (DESIGN.md §9.4): dark only. One `schemeDark` ships; the light
// reference values stay recorded in DESIGN.md §2 until a light scheme earns its
// build. Rosé Pine is retired. Scalars (radius, type, motion, spacing) are
// declared straight as sys tokens — a ref ramp of scalars would be ceremony with
// no second consumer.
// ──────────────────────────────────────────────────────────────────────────────
QtObject {
    id: theme

    // ══ ref.* — primitive colour ramps ═══════════════════════════════════════
    // Every neutral keeps a subtle cool cast (never pure grey); the accent is the
    // One UI blue. Named by ramp position, not by role — roles live in the scheme.
    component RefPalette: QtObject {
        // Near-black neutral ramp (dark scheme). `dN` are derived intermediate
        // steps the SESL table does not name but the surfaces need.
        readonly property color night: "#010102"     // window / canvas
        readonly property color d1: "#0d0d0f"         // derived — badge/input floors
        readonly property color card: "#17171a"       // content card
        readonly property color d2: "#1d1d20"         // derived — strong surface
        readonly property color elevated: "#2d2d30"   // elevated surface
        readonly property color divider: "#3a3a3d"    // hairline (used sparingly)
        readonly property color d4: "#55555a"         // derived — strong divider
        readonly property color textHi: "#fafaff"     // primary text
        readonly property color textLo: "#99999e"     // secondary text
        // One UI accent ramp — interactive/active only.
        readonly property color accent: "#387aff"
        readonly property color accentLink: "#598fff"
        readonly property color accentPressed: "#376fde"
        readonly property color accentInk: "#fcfcff"
        // Semantic ramp (SESL dark).
        readonly property color danger: "#fc6c65"
        readonly property color success: "#58db9c"
        readonly property color warning: "#fc864c"
        // The one warm exception, spent on the star a favourite wears.
        readonly property color favorite: "#f2c55c"
    }
    readonly property RefPalette ref: RefPalette {}

    // ══ scheme — semantic roles as data ══════════════════════════════════════
    // `required` on every role means a ColorScheme instance must set them all:
    // the day a light scheme is added, the compiler refuses a scheme that drops a
    // role or its foreground pair. Opaque, stackable surfaces (canvas/card/
    // elevated, accent, the semantics, the danger banner) carry an explicit
    // foreground token; translucent state washes (surface*/hover/selected/fills)
    // do not mint six identical whites — their foreground is `text`/`textMuted`
    // by contract (documented in CLAUDE.md).
    //
    // NAMING: the sealed contract (DESIGN §6.9/§9.1) calls these pairs `on*`
    // (`onAccent`, `onCanvas`, …, the Material convention). QML reserves the
    // `on<Capital>` identifier namespace for signal handlers, so a property named
    // `onAccent` is illegal. The pairs therefore ship as `<surface>Ink`
    // (`accentInk` = the contract's `onAccent`, `canvasInk` = `onCanvas`, …):
    // same pairs, same meaning, a spelling QML accepts.
    component ColorScheme: QtObject {
        required property color canvas
        required property color canvasInk
        required property color card
        required property color cardInk
        required property color elevated
        required property color elevatedInk
        required property color text
        required property color textMuted
        required property color divider
        required property color dividerStrong
        required property color accent
        required property color accentInk
        required property color accentLink
        required property color accentPressed
        required property color focusRing
        required property color danger
        required property color dangerInk
        required property color success
        required property color successInk
        required property color warning
        required property color warningInk
        required property color scrim
        // Translucent state washes (foreground = text/textMuted by contract).
        required property color surface
        required property color surfaceStrong
        required property color surfaceHover
        required property color surfaceSelected
        required property color inputFill
        required property color inputFillFocus
        required property color inputBorder
        required property color controlFill
        required property color badgeFill
        required property color badgeAccentFill
        // Entry-kind glyph tints.
        required property color glyphDirectory
        required property color glyphSymlink
        required property color glyphFile
        // Favourite (the warm exception) + its badge floor.
        required property color favorite
        required property color favoriteBadgeFill
        // Danger banner (fill / outline / text-on-fill).
        required property color dangerFill
        required property color dangerBorder
        required property color dangerFillInk
        // Canvas backdrop gradient (near-black; doctrine keeps the near-black).
        required property color gradientStart
        required property color gradientMid
        required property color gradientEnd
        // Glass — the recipe rebuild is S2; only the tint neutralises now so the
        // colour family stops fighting the near-black. Border/highlight unchanged.
        required property color glassTint
        required property color glassBorder
        required property color glassHighlight
    }

    readonly property ColorScheme schemeDark: ColorScheme {
        canvas: theme.ref.night
        canvasInk: theme.ref.textHi
        card: theme.ref.card
        cardInk: theme.ref.textHi
        elevated: theme.ref.elevated
        elevatedInk: theme.ref.textHi
        text: theme.ref.textHi
        textMuted: theme.ref.textLo
        divider: theme.ref.divider
        dividerStrong: theme.ref.d4
        accent: theme.ref.accent
        accentInk: theme.ref.accentInk
        accentLink: theme.ref.accentLink
        accentPressed: theme.ref.accentPressed
        // Keyboard focus is an active state — the ring is the accent's lit tint.
        focusRing: theme.ref.accentLink
        danger: theme.ref.danger
        dangerInk: theme.ref.night
        success: theme.ref.success
        successInk: theme.ref.night
        warning: theme.ref.warning
        warningInk: theme.ref.night
        scrim: "#66000000"
        // Washes re-based off the neutral ramp (were a blue-grey cast).
        surface: "#d917171a"
        surfaceStrong: "#f01d1d20"
        surfaceHover: "#2a2d2d30"
        // Selection reads as a soft accent wash — the One UI "selected" language.
        surfaceSelected: "#33387aff"
        inputFill: "#4d0d0d0f"
        inputFillFocus: "#661a1a1e"
        inputBorder: "#24505055"
        controlFill: "#2b2d2d30"
        badgeFill: "#28292930"
        // Current/selected row wash: accent-tinted (alpha nudged up — blue reads
        // lighter than the old white at the same opacity).
        badgeAccentFill: "#26387aff"
        glyphDirectory: "#33323237"
        glyphSymlink: "#2c3e3e43"
        glyphFile: "#2a2e2e33"
        favorite: theme.ref.favorite
        favoriteBadgeFill: "#b30d0d0f"
        // Danger banner re-tinted around the new danger red.
        dangerFill: "#4d571f1c"
        dangerBorder: "#8ad4736c"
        dangerFillInk: "#ffdad6"
        gradientStart: "#010102"
        gradientMid: "#08080b"
        gradientEnd: "#020204"
        glassTint: "#a61c1c1f"
        glassBorder: "#5cffffff"
        glassHighlight: "#2effffff"
    }

    // The active scheme — the single switch point. Bindings re-evaluate once if
    // this ever flips to a light scheme (verified cheap).
    readonly property ColorScheme scheme: schemeDark

    // ══ sys.* colours — flat, what apps consume ══════════════════════════════
    // The API shape is unchanged (`CelestinaTheme.canvas`); every value is just a
    // read of the active scheme.
    readonly property color canvas: scheme.canvas
    readonly property color canvasInk: scheme.canvasInk
    readonly property color card: scheme.card
    readonly property color cardInk: scheme.cardInk
    readonly property color elevated: scheme.elevated
    readonly property color elevatedInk: scheme.elevatedInk
    readonly property color text: scheme.text
    readonly property color textMuted: scheme.textMuted
    readonly property color divider: scheme.divider
    readonly property color dividerStrong: scheme.dividerStrong
    readonly property color accent: scheme.accent
    readonly property color accentInk: scheme.accentInk
    readonly property color accentLink: scheme.accentLink
    readonly property color accentPressed: scheme.accentPressed
    readonly property color focusRing: scheme.focusRing
    readonly property color danger: scheme.danger
    readonly property color dangerInk: scheme.dangerInk
    readonly property color success: scheme.success
    readonly property color successInk: scheme.successInk
    readonly property color warning: scheme.warning
    readonly property color warningInk: scheme.warningInk
    readonly property color scrim: scheme.scrim
    readonly property color surface: scheme.surface
    readonly property color surfaceStrong: scheme.surfaceStrong
    readonly property color surfaceHover: scheme.surfaceHover
    readonly property color surfaceSelected: scheme.surfaceSelected
    readonly property color inputFill: scheme.inputFill
    readonly property color inputFillFocus: scheme.inputFillFocus
    readonly property color inputBorder: scheme.inputBorder
    readonly property color controlFill: scheme.controlFill
    readonly property color badgeFill: scheme.badgeFill
    readonly property color badgeAccentFill: scheme.badgeAccentFill
    readonly property color glyphDirectory: scheme.glyphDirectory
    readonly property color glyphSymlink: scheme.glyphSymlink
    readonly property color glyphFile: scheme.glyphFile
    readonly property color favorite: scheme.favorite
    readonly property color favoriteBadgeFill: scheme.favoriteBadgeFill
    readonly property color dangerFill: scheme.dangerFill
    readonly property color dangerBorder: scheme.dangerBorder
    readonly property color dangerFillInk: scheme.dangerFillInk
    readonly property color gradientStart: scheme.gradientStart
    readonly property color gradientMid: scheme.gradientMid
    readonly property color gradientEnd: scheme.gradientEnd
    readonly property color glassTint: scheme.glassTint
    readonly property color glassBorder: scheme.glassBorder
    readonly property color glassHighlight: scheme.glassHighlight

    // ── Typography ───────────────────────────────────────────────────────────
    // Inter Variable ships inside the module (celestina-style/fonts, OFL) so the
    // suite renders in its own typeface instead of whatever fontconfig resolves.
    // The font is compiled into each app's qrc; where it is not (the shell's
    // output chooser imports the plain module at runtime), sansFamily degrades to
    // an empty family and Qt picks the application default — the same honest
    // fallback posture as fallbackIcon().
    readonly property FontLoader interLoader: FontLoader {
        source: "qrc:/qt/qml/CelestinaStyle/fonts/InterVariable.ttf"
    }
    readonly property string sansFamily: interLoader.status === FontLoader.Ready
                                         ? interLoader.name
                                         : ""
    readonly property string monoFamily: "monospace"
    // OpenType tabular figures — equal-width digits for panel numerics that must
    // not jitter as values change (sizes, transfer counts). Bind into
    // Text.font.features on the numeric labels that need it.
    readonly property var fontFeaturesTabular: ({ "tnum": 1 })

    // Type roles (One UI: large, comfortable, semibold titles over heavy bolds).
    // Starting px; tuned per surface with screenshots in the visual phases.
    readonly property int fontMini: 10
    readonly property int fontCaption: 11
    readonly property int fontRowSecondary: 12       // list-row subtitle
    readonly property int fontBody: 13               // dialog/body text
    readonly property int fontRowTitle: 15           // list-row title, dialog headings
    readonly property int fontTitle: 17              // dialog title (ready; S4 dialogs)
    readonly property int fontHeaderCollapsed: 20    // collapsed big header
    readonly property int fontHeaderExpanded: 30     // expanded big header (ready; CP4/S4)
    readonly property int fontDisplay: 34            // display (ready)

    readonly property int weightRegular: Font.Normal    // 400 — body
    readonly property int weightMedium: Font.Medium     // 500 — kept until components settle to 400/600 (S4)
    readonly property int weightDemiBold: Font.DemiBold // 600 — titles

    // ── Radius scale ───────────────────────────────────────────────────────────
    // One UI's generous rounding: radius scales down with element size.
    // radiusButton/radiusInput are ready for CelestinaButton/TextField to adopt in
    // S4 (the button-emphasis + input-anatomy work); S1 leaves them defined.
    readonly property int radiusSm: 12       // controls, chips, glyph tiles, rows, banners
    readonly property int radiusMd: 20       // glass menus / floating surfaces
    readonly property int radiusButton: 18   // ready — filled/tonal buttons (S4)
    readonly property int radiusInput: 22    // ready — search/text field (S4)
    readonly property int radiusLg: 26       // dialogs, popup menus, grouped cards
    readonly property int radiusPill: 9999   // full capsule

    // ── Spacing scale (4-based) ──────────────────────────────────────────────
    readonly property int spaceXs: 4
    readonly property int spaceSm: 8
    readonly property int spaceMd: 12
    readonly property int spaceLg: 16
    readonly property int spaceXl: 20
    readonly property int space2xl: 24
    readonly property int space3xl: 32

    // ── Control metrics ──────────────────────────────────────────────────────
    readonly property int controlHeight: 38
    readonly property int controlHeightLg: 42
    readonly property int rowHeight: 54
    readonly property int glyphTile: 34
    readonly property int iconSm: 18
    readonly property int iconMd: 19

    // ── Motion ─────────────────────────────────────────────────────────────────
    // Duration ladder (DESIGN §6.7): dialogs/quick 100, normal 200, expressive
    // 350, hard ceiling 500. The bezier tokens are the One UI curves, ready for
    // S2's motion retune (recoil, opacity→linear, bezier adoption); S1 keeps the
    // existing enum easing so the motion delta stays at "small".
    readonly property int motionFast: 100
    readonly property int motionNormal: 200
    readonly property int motionSlow: 350
    readonly property int motionCeiling: 500
    // How long a drag must rest on a folder before it springs open. Long enough
    // that crossing one on the way somewhere else never opens it, short enough
    // that deliberately waiting does not feel broken.
    readonly property int springDelay: 800

    // The official One UI curve (fast start, long settle) and its two sine
    // variants, as bezier control points for `easing.bezierCurve`. Ready for S2;
    // consume with `easing.type: Easing.Bezier; easing.bezierCurve: easeOneUi`.
    readonly property var easeOneUi: [0.22, 0.25, 0, 1, 1, 1]
    readonly property var easeSineInOut80: [0.33, 0, 0.2, 1, 1, 1]
    readonly property var easeSineInOut90: [0.33, 0, 0.1, 1, 1, 1]

    // Enum easing vocabulary in use today (retuned to the bezier curves in S2).
    // easeEmphasized is the One UI "settle" (mild overshoot) on popup reveals.
    readonly property int easeStandard: Easing.OutCubic
    readonly property int easeDecelerate: Easing.OutQuint
    readonly property int easeEmphasized: Easing.OutBack
    readonly property int easeExit: Easing.InCubic
    readonly property real overshoot: 1.15

    // ── Glass parameters (scalars) ─────────────────────────────────────────────
    // Backdrop-blur knobs for GlassSurface. The colours (glassTint/Border/
    // Highlight) live in the scheme; the full glass-v2 recipe is S2.
    readonly property real glassBlur: 0.60
    readonly property int glassBlurMax: 30
    readonly property real glassSaturation: 0.14
    readonly property real glassSampleScale: 0.66
    readonly property int glassSampleMargin: 20

    // ── Component knobs (comp.*) ───────────────────────────────────────────────
    // Per-component geometry, kept minimal — only the glass menu needs it today.
    readonly property int compMenuWidth: 232
    readonly property int compMenuPadding: 6
    readonly property int compMenuMargins: 24

    // ── Icons ──────────────────────────────────────────────────────────────────
    // Minimal monochrome freedesktop-name fallbacks bundled with the module.
    readonly property string fallbackIconRoot: "qrc:/qt/qml/CelestinaStyle/icons/"

    function fallbackIcon(name) {
        return fallbackIconRoot + name + ".svg"
    }
}
