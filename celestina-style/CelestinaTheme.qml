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
//   comp*    the few component anatomy metrics that every app must share.
//
// Sealed (DESIGN.md §9.4): dark only. One `schemeDark` ships; the light
// reference values stay recorded in DESIGN.md §2 until a light scheme earns its
// build. Rosé Pine is retired. Scalars (radius, type, motion, spacing) are
// declared straight as sys tokens — a ref ramp of scalars would be ceremony with
// no second consumer.
// ──────────────────────────────────────────────────────────────────────────────
QtObject {
    id: theme

    // Colour recipes live here rather than in consumers. `withAlpha` creates a
    // semantic wash from a base colour; `multiplyAlpha` preserves the alpha of
    // an existing colour (the lit glass edge uses it); `mixColors` derives a
    // tint while keeping the base accent as the only hue dial.
    function withAlpha(value, alpha) {
        return Qt.rgba(value.r, value.g, value.b,
                       Math.max(0, Math.min(1, alpha)))
    }

    function multiplyAlpha(value, factor) {
        return withAlpha(value, value.a * factor)
    }

    function mixColors(sourceColor, targetColor, amount) {
        const t = Math.max(0, Math.min(1, amount))
        return Qt.rgba(sourceColor.r + (targetColor.r - sourceColor.r) * t,
                       sourceColor.g + (targetColor.g - sourceColor.g) * t,
                       sourceColor.b + (targetColor.b - sourceColor.b) * t,
                       sourceColor.a + (targetColor.a - sourceColor.a) * t)
    }

    // ── Icon gradient recipe (OKLCH) ───────────────────────────────────────
    // A content icon is painted as a soft two-stop wash of its own tone. The
    // arithmetic runs in OKLCH and not in HSL, and that is the whole reason this
    // block exists: darkening a warm tone in HSL walks it into olive — an amber
    // folder ended up yellow-green at the bottom — because HSL's "lightness"
    // ignores how the eye reads a hue. OKLCH keeps perceived hue while lightness
    // moves, so every tone deepens into itself.
    //
    // The wash is deliberately small: a little lift at the top, a little drop
    // and chroma at the bottom, and a few degrees of turn so it reads as colour
    // rather than as shading. Bigger numbers than these stop looking like a
    // material and start looking like a poster.
    readonly property real iconGradientLift: 0.05     // L, hacia arriba
    readonly property real iconGradientDrop: 0.05     // L, hacia abajo
    readonly property real iconGradientTurn: 5        // grados de giro, ±
    readonly property real iconGradientChroma: 0.05   // croma que gana el bajo

    function srgbToLinear(channel) {
        return channel <= 0.04045 ? channel / 12.92
                                  : Math.pow((channel + 0.055) / 1.055, 2.4)
    }

    function linearToSrgb(channel) {
        return channel <= 0.0031308 ? channel * 12.92
                                    : 1.055 * Math.pow(channel, 1 / 2.4) - 0.055
    }

    // Devuelve [L, C, H] — H en grados.
    function toOklch(value) {
        const r = srgbToLinear(value.r)
        const g = srgbToLinear(value.g)
        const b = srgbToLinear(value.b)
        const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
        const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
        const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
        const lightness = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s
        const greenRed = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s
        const blueYellow = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s
        return [lightness, Math.hypot(greenRed, blueYellow),
                (Math.atan2(blueYellow, greenRed) * 180 / Math.PI + 360) % 360]
    }

    // Los tres canales sRGB de un OKLCH, sin acotar: hace falta verlos crudos
    // para saber si el color cabe en la pantalla.
    function oklchChannels(lightness, chroma, hue) {
        const radians = hue * Math.PI / 180
        const greenRed = chroma * Math.cos(radians)
        const blueYellow = chroma * Math.sin(radians)
        const l = Math.pow(lightness + 0.3963377774 * greenRed
                           + 0.2158037573 * blueYellow, 3)
        const m = Math.pow(lightness - 0.1055613458 * greenRed
                           - 0.0638541728 * blueYellow, 3)
        const s = Math.pow(lightness - 0.0894841775 * greenRed
                           - 1.2914855480 * blueYellow, 3)
        return [linearToSrgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
                linearToSrgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
                linearToSrgb(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s)]
    }

    function channelsFitSrgb(channels) {
        for (let index = 0; index < 3; ++index)
            if (channels[index] < -0.002 || channels[index] > 1.002)
                return false
        return true
    }

    function fromOklch(lightness, chroma, hue, alpha) {
        // Un color que no cabe en sRGB no se puede pintar, y recortarlo canal a
        // canal **gira el tono** — el azul de carpeta se iba 9° al iluminarlo,
        // que es la misma clase de fallo que esta receta vino a arreglar. Así
        // que en vez de recortar se baja el croma hasta que entra: se pierde
        // algo de viveza, nunca la identidad del color.
        let channels = oklchChannels(lightness, chroma, hue)
        if (!channelsFitSrgb(channels)) {
            let low = 0
            let high = chroma
            for (let step = 0; step < 12; ++step) {
                const middle = (low + high) / 2
                if (channelsFitSrgb(oklchChannels(lightness, middle, hue)))
                    low = middle
                else
                    high = middle
            }
            channels = oklchChannels(lightness, low, hue)
        }
        const clamp = function(channel) {
            return Math.max(0, Math.min(1, channel))
        }
        return Qt.rgba(clamp(channels[0]), clamp(channels[1]), clamp(channels[2]),
                       alpha)
    }

    function iconGradientTop(value) {
        const lch = toOklch(value)
        return fromOklch(Math.min(1, lch[0] + iconGradientLift),
                         lch[1] * (1 - iconGradientChroma * 0.5),
                         (lch[2] - iconGradientTurn + 360) % 360,
                         value.a)
    }

    function iconGradientBottom(value) {
        const lch = toOklch(value)
        return fromOklch(Math.max(0, lch[0] - iconGradientDrop),
                         lch[1] * (1 + iconGradientChroma),
                         (lch[2] + iconGradientTurn) % 360,
                         value.a)
    }

    // Anatomía de la carpeta, compartida por cualquier consumidor que la pinte:
    // el redondeo, cuánto ocupa la pestaña y cuánto dura su hombro. Son del
    // tema porque definen la silueta de la suite, no una pantalla concreta.
    readonly property real iconFolderCorner: 0.10
    readonly property real iconFolderTab: 0.52
    readonly property real iconFolderShoulder: 0.12
    // La hoja que asoma y la tinta del emblema: casi blanca y cálida, para que
    // no compita con el tono de la carpeta ni se confunda con el texto.
    readonly property color iconSheet: "#fbf7ee"

    // La tinta del emblema forma par con el bolsillo sobre el que se pinta, no
    // con la ventana: sobre un tono claro —un ámbar de favoritos, el plata de
    // los archivos— una tinta casi blanca se borra, y el guard de contraste lo
    // caza. Así que por encima de cierta claridad se cambia por una tinta
    // profunda del mismo tono, que es la misma regla de pares que el resto del
    // sistema.
    readonly property real iconEmblemInkThreshold: 0.62
    readonly property real iconEmblemInkLightness: 0.26

    function iconEmblemInk(value) {
        const pocket = toOklch(iconGradientBottom(value))
        if (pocket[0] <= iconEmblemInkThreshold)
            return withAlpha(iconSheet, value.a)
        return fromOklch(iconEmblemInkLightness, pocket[1] * 0.85, pocket[2],
                         value.a)
    }

    // La hoja sigue la misma regla contra el fondo de la carpeta: en un tono
    // claro, un papel casi blanco sobre un fondo casi blanco no es papel, es
    // nada. Ahí se vuelve una lámina profunda del mismo tono.
    function iconSheetTone(value) {
        const backdrop = toOklch(iconBackdropTone(value))
        if (backdrop[0] <= iconEmblemInkThreshold)
            return withAlpha(iconSheet, value.a)
        return fromOklch(iconEmblemInkLightness, backdrop[1] * 0.85, backdrop[2],
                         value.a)
    }

    // El fondo de la carpeta: el mismo tono, asentado. También en OKLCH, para
    // que no se vaya de familia respecto al bolsillo que lleva delante.
    readonly property real iconBackdropDrop: 0.13

    function iconBackdropTone(value) {
        const lch = toOklch(value)
        return fromOklch(Math.max(0, lch[0] - iconBackdropDrop),
                         lch[1] * (1 + iconGradientChroma * 0.5),
                         lch[2], value.a)
    }

    // Accent recipe. Change `ref.accent` once; links, interaction states and
    // translucent selection/disabled roles all follow automatically.
    readonly property real accentLinkMix: 0.24
    readonly property real accentHoverMix: 0.08
    readonly property real accentPressedMix: 0.14
    readonly property real accentSelectedOpacity: 0.20
    readonly property real accentMarqueeOpacity: 0.18
    readonly property real accentBadgeOpacity: 0.15
    readonly property real accentSoftOpacity: 0.14
    readonly property real accentSoftBorderOpacity: 0.18
    readonly property real accentDisabledInkOpacity: 0.75

    // ══ ref.* — primitive colour ramps ═══════════════════════════════════════
    // Every neutral keeps a subtle cool cast (never pure grey); the accent is the
    // One UI blue. Named by ramp position, not by role — roles live in the scheme.
    component RefPalette: QtObject {
        // Near-black neutral ramp (dark scheme). `dN` are derived intermediate
        // steps the SESL table does not name but the surfaces need.
        readonly property color night: "#050608"      // window / canvas
        readonly property color d1: "#090b0f"         // input / backdrop floor
        readonly property color card: "#14171c"       // grouped tonal block
        readonly property color d2: "#1a1e25"         // strong tonal surface
        readonly property color elevated: "#222831"   // elevated surface
        readonly property color divider: "#16ffffff"  // quiet hairline
        readonly property color d4: "#24ffffff"        // strong hairline
        readonly property color textHi: "#f7f8fc"     // primary text
        readonly property color textLo: "#9ba3af"     // secondary text
        readonly property color textFaint: "#78818e"  // labels / metadata
        // One UI accent seed. Every state is derived from this one value in the
        // active scheme below; changing the suite accent is therefore one edit.
        readonly property color accent: "#3e91ff"
        // Ink painted on the bright accent must be dark enough for body-sized
        // labels and icons. `accentLift` remains the cool white used to derive
        // lighter accent states; it is not an on-accent foreground.
        readonly property color accentInk: "#050608"
        readonly property color accentLift: "#fcfcff"
        // Semantic ramp (SESL dark).
        readonly property color danger: "#ff746d"
        readonly property color success: "#59dc9e"
        readonly property color warning: "#fc864c"
        // The one warm exception, spent on the star a favourite wears.
        readonly property color favorite: "#f2c55c"
        // Code colours. Muted on purpose: an editor is read for minutes at a
        // time, and four saturated hues fighting each other is what makes
        // syntax highlighting tiring. Each clears 4.5:1 on the input fill.
        readonly property color codeComment: "#8d9bab"
        readonly property color codeString: "#8fd3ac"
        readonly property color codeNumber: "#f0b083"
        readonly property color codeKeyword: "#b9a6f0"
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
        required property color textFaint
        required property color divider
        required property color dividerStrong
        required property color accent
        required property color accentInk
        required property color accentLink
        required property color accentHover
        required property color accentPressed
        required property color focusRing
        required property color danger
        required property color dangerInk
        required property color success
        required property color successInk
        required property color warning
        required property color warningInk
        required property color codeComment
        required property color codeString
        required property color codeNumber
        required property color codeKeyword
        required property color scrim
        // Translucent state washes (foreground = text/textMuted by contract).
        required property color surface
        required property color surfaceStrong
        required property color surfaceHover
        required property color surfaceSelected
        required property color selectionMarquee
        required property color inputFill
        required property color inputFillFocus
        required property color inputBorder
        required property color controlFill
        required property color badgeFill
        required property color badgeAccentFill
        required property color accentSoft
        required property color accentSoftBorder
        required property color successSoft
        // A primary action that remains identifiable while disabled.
        required property color accentDisabledFill
        required property color accentDisabledInk
        // Dark media affordances painted over thumbnails.
        required property color mediaScrim
        required property color mediaScrimInk
        required property color mediaSurfaceStart
        required property color mediaSurfaceMid
        required property color mediaSurfaceEnd
        required property color mediaSurfaceInk
        required property color mediaArtworkStart
        required property color mediaArtworkMid
        required property color mediaArtworkEnd
        required property color mediaArtworkInk
        required property color mediaProgress
        required property color mediaProgressTrack
        // Entry-kind glyph tints.
        required property color glyphDirectory
        required property color glyphSymlink
        required property color glyphFile
        required property color glyphNavigation
        required property color glyphDevice
        // Closed user-selectable icon accents. They share luminance and
        // saturation so a custom mark stays in the suite even when its hue is
        // intentionally distinctive.
        required property color glyphAccentBlue
        required property color glyphAccentCyan
        required property color glyphAccentGreen
        required property color glyphAccentViolet
        required property color glyphAccentCoral
        required property color glyphAccentAmber
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
        // Glass: regular floating tint, stronger modal tint, a lighter tint for
        // compositor-owned blur, restrained lit edge and a dark outline.
        required property color glassTint
        required property color glassTintStrong
        required property color compositorGlassTint
        required property color compositorGlassFallback
        required property color glassBorder
        required property color glassHighlight
        required property color glassOutline
        // Elevation — the drop shadow under floating layers (L2). Soft and light
        // per the depth doctrine; on near-black it reads as a faint dark halo.
        required property color shadow
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
        textFaint: theme.ref.textFaint
        divider: theme.ref.divider
        dividerStrong: theme.ref.d4
        accent: theme.ref.accent
        accentInk: theme.ref.accentInk
        accentLink: theme.mixColors(theme.ref.accent, theme.ref.accentLift,
                                    theme.accentLinkMix)
        accentHover: theme.mixColors(theme.ref.accent, theme.ref.accentLift,
                                     theme.accentHoverMix)
        accentPressed: theme.mixColors(theme.ref.accent, theme.ref.night,
                                       theme.accentPressedMix)
        // Keyboard focus is an active state — the ring is the accent's lit tint.
        focusRing: theme.mixColors(theme.ref.accent, theme.ref.accentLift,
                                   theme.accentLinkMix)
        danger: theme.ref.danger
        dangerInk: theme.ref.night
        success: theme.ref.success
        successInk: theme.ref.night
        warning: theme.ref.warning
        warningInk: theme.ref.night
        codeComment: theme.ref.codeComment
        codeString: theme.ref.codeString
        codeNumber: theme.ref.codeNumber
        codeKeyword: theme.ref.codeKeyword
        scrim: "#73000000"
        // State layers are translucent; actual work/group surfaces use the
        // opaque card/elevated roles through CelestinaSurface.
        surface: "#d914171c"
        surfaceStrong: "#f01a1e25"
        surfaceHover: "#0dffffff"
        // Selection reads as a soft accent wash — the One UI "selected" language.
        surfaceSelected: theme.withAlpha(theme.ref.accent,
                                         theme.accentSelectedOpacity)
        selectionMarquee: theme.withAlpha(theme.ref.accent,
                                          theme.accentMarqueeOpacity)
        inputFill: "#8c14171c"
        inputFillFocus: "#b31a1e25"
        inputBorder: "#16ffffff"
        controlFill: "#0effffff"
        badgeFill: "#0effffff"
        // Current/selected row wash: accent-tinted (alpha nudged up — blue reads
        // lighter than the old white at the same opacity).
        badgeAccentFill: theme.withAlpha(theme.ref.accent,
                                         theme.accentBadgeOpacity)
        accentSoft: theme.withAlpha(theme.ref.accent,
                                    theme.accentSoftOpacity)
        accentSoftBorder: theme.withAlpha(
                              theme.mixColors(theme.ref.accent,
                                              theme.ref.accentLift,
                                              theme.accentLinkMix),
                              theme.accentSoftBorderOpacity)
        successSoft: "#1c59dc9e"
        accentDisabledFill: theme.withAlpha(theme.ref.accent,
                                            theme.accentSelectedOpacity)
        accentDisabledInk: theme.withAlpha(theme.ref.accent,
                                           theme.accentDisabledInkOpacity)
        // Artwork is untrusted visual input. This floor keeps foreground text
        // readable even when a cover is almost white.
        mediaScrim: "#cc000000"
        mediaScrimInk: theme.ref.textHi
        mediaSurfaceStart: "#242d3c"
        mediaSurfaceMid: "#222634"
        mediaSurfaceEnd: "#29232d"
        mediaSurfaceInk: theme.ref.textHi
        mediaArtworkStart: "#9eb9d3"
        mediaArtworkMid: "#536d8e"
        mediaArtworkEnd: "#d39a7f"
        mediaArtworkInk: theme.ref.textHi
        mediaProgress: "#e7edf6"
        mediaProgressTrack: "#4de7edf6"
        // Icon ink stays within one cool family by default: vivid blue marks
        // content folders, silver-blue marks plain files, slate marks
        // navigation/sidebar chrome and cyan marks connected hardware.
        glyphDirectory: theme.mixColors(theme.ref.accent,
                                        theme.ref.accentLift,
                                        theme.accentLinkMix)
        glyphFile: "#a9b5c5"
        glyphSymlink: "#a391e2"
        glyphNavigation: "#8fa3bb"
        glyphDevice: "#68c3d4"
        glyphAccentBlue: "#6ea8ff"
        glyphAccentCyan: "#68c3d4"
        glyphAccentGreen: "#72cfa3"
        glyphAccentViolet: "#a391e2"
        glyphAccentCoral: "#e88d82"
        glyphAccentAmber: "#dcb36a"
        favorite: theme.ref.favorite
        favoriteBadgeFill: "#b3090b0f"
        // Danger banner re-tinted around the new danger red.
        dangerFill: "#4d571f1c"
        dangerBorder: "#8ad4736c"
        dangerFillInk: "#ffdad6"
        gradientStart: "#080a0f"
        gradientMid: "#0b0e14"
        gradientEnd: "#08090d"
        // Dense enough for desktop text, but translucent enough for the
        // reference's pink/green backdrop colour to remain visible through the
        // blur. Strong is reserved for modal readability.
        glassTint: "#991a1e25"
        glassTintStrong: "#bd1a1e25"
        // Wallpaper is untrusted visual input. Even with compositor blur the
        // tint must provide a contrast floor; the fallback is denser when the
        // host cannot arm blur at all.
        compositorGlassTint: "#e61a1e25"
        compositorGlassFallback: "#f51a1e25"
        glassBorder: "#24ffffff"
        glassHighlight: "#2effffff"
        glassOutline: "#4d000000"
        shadow: "#78000000"
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
    readonly property color textFaint: scheme.textFaint
    readonly property color divider: scheme.divider
    readonly property color dividerStrong: scheme.dividerStrong
    readonly property color accent: scheme.accent
    readonly property color accentInk: scheme.accentInk
    readonly property color accentLink: scheme.accentLink
    readonly property color accentHover: scheme.accentHover
    readonly property color accentPressed: scheme.accentPressed
    readonly property color focusRing: scheme.focusRing
    readonly property color danger: scheme.danger
    readonly property color dangerInk: scheme.dangerInk
    readonly property color success: scheme.success
    readonly property color successInk: scheme.successInk
    readonly property color warning: scheme.warning
    readonly property color warningInk: scheme.warningInk
    // Syntax colours, consumed by an editor surface over `inputFill`.
    readonly property color codeComment: scheme.codeComment
    readonly property color codeString: scheme.codeString
    readonly property color codeNumber: scheme.codeNumber
    readonly property color codeKeyword: scheme.codeKeyword
    readonly property color scrim: scheme.scrim
    readonly property color surface: scheme.surface
    readonly property color surfaceStrong: scheme.surfaceStrong
    readonly property color surfaceHover: scheme.surfaceHover
    readonly property color surfaceSelected: scheme.surfaceSelected
    readonly property color selectionMarquee: scheme.selectionMarquee
    readonly property color inputFill: scheme.inputFill
    readonly property color inputFillFocus: scheme.inputFillFocus
    readonly property color inputBorder: scheme.inputBorder
    readonly property color controlFill: scheme.controlFill
    readonly property color badgeFill: scheme.badgeFill
    readonly property color badgeAccentFill: scheme.badgeAccentFill
    readonly property color accentSoft: scheme.accentSoft
    readonly property color accentSoftBorder: scheme.accentSoftBorder
    readonly property color successSoft: scheme.successSoft
    readonly property color accentDisabledFill: scheme.accentDisabledFill
    readonly property color accentDisabledInk: scheme.accentDisabledInk
    readonly property color mediaScrim: scheme.mediaScrim
    readonly property color mediaScrimInk: scheme.mediaScrimInk
    readonly property color mediaSurfaceStart: scheme.mediaSurfaceStart
    readonly property color mediaSurfaceMid: scheme.mediaSurfaceMid
    readonly property color mediaSurfaceEnd: scheme.mediaSurfaceEnd
    readonly property color mediaSurfaceInk: scheme.mediaSurfaceInk
    readonly property color mediaArtworkStart: scheme.mediaArtworkStart
    readonly property color mediaArtworkMid: scheme.mediaArtworkMid
    readonly property color mediaArtworkEnd: scheme.mediaArtworkEnd
    readonly property color mediaArtworkInk: scheme.mediaArtworkInk
    readonly property color mediaProgress: scheme.mediaProgress
    readonly property color mediaProgressTrack: scheme.mediaProgressTrack
    readonly property color glyphDirectory: scheme.glyphDirectory
    readonly property color glyphSymlink: scheme.glyphSymlink
    readonly property color glyphFile: scheme.glyphFile
    readonly property color glyphNavigation: scheme.glyphNavigation
    readonly property color glyphDevice: scheme.glyphDevice
    readonly property color glyphAccentBlue: scheme.glyphAccentBlue
    readonly property color glyphAccentCyan: scheme.glyphAccentCyan
    readonly property color glyphAccentGreen: scheme.glyphAccentGreen
    readonly property color glyphAccentViolet: scheme.glyphAccentViolet
    readonly property color glyphAccentCoral: scheme.glyphAccentCoral
    readonly property color glyphAccentAmber: scheme.glyphAccentAmber
    readonly property color favorite: scheme.favorite
    readonly property color favoriteBadgeFill: scheme.favoriteBadgeFill
    readonly property color dangerFill: scheme.dangerFill
    readonly property color dangerBorder: scheme.dangerBorder
    readonly property color dangerFillInk: scheme.dangerFillInk
    readonly property color gradientStart: scheme.gradientStart
    readonly property color gradientMid: scheme.gradientMid
    readonly property color gradientEnd: scheme.gradientEnd
    readonly property color glassTint: scheme.glassTint
    readonly property color glassTintStrong: scheme.glassTintStrong
    readonly property color compositorGlassTint: scheme.compositorGlassTint
    readonly property color compositorGlassFallback: scheme.compositorGlassFallback
    readonly property color glassBorder: scheme.glassBorder
    readonly property color glassHighlight: scheme.glassHighlight
    readonly property color glassOutline: scheme.glassOutline
    readonly property color shadow: scheme.shadow
    // Compositing constants are scheme invariant; keeping them here prevents
    // visual implementations from spelling ad-hoc colour literals.
    readonly property color clear: "#00000000"
    readonly property color opaqueMask: "#ffffffff"

    // Stable keys are persisted by Siderita; colours remain tokens so a later
    // palette retune updates every customized item without rewriting config.
    readonly property var iconAccentKeys: [
        "blue", "cyan", "green", "violet", "coral", "amber"
    ]

    function iconAccentColor(key) {
        switch (key) {
        case "blue": return glyphAccentBlue
        case "cyan": return glyphAccentCyan
        case "green": return glyphAccentGreen
        case "violet": return glyphAccentViolet
        case "coral": return glyphAccentCoral
        case "amber": return glyphAccentAmber
        default: return clear
        }
    }

    function iconAccentLabel(key) {
        switch (key) {
        case "blue": return "Azul"
        case "cyan": return "Cian"
        case "green": return "Verde"
        case "violet": return "Violeta"
        case "coral": return "Coral"
        case "amber": return "Ámbar"
        default: return "Automático"
        }
    }

    // ── Typography ───────────────────────────────────────────────────────────
    // Inter Variable ships inside the module (celestina-style/fonts, OFL) so the
    // suite renders in its own typeface instead of whatever fontconfig resolves.
    // The font is compiled into each app's qrc; where it is not (the shell's
    // output chooser imports the plain module at runtime), sansFamily degrades to
    // an empty family and Qt picks the application default — the same honest
    // fallback posture as fallbackIcon().
    readonly property FontLoader interLoader: FontLoader {
        source: Qt.resolvedUrl(".").toString().startsWith("file:")
                ? Qt.resolvedUrl("fonts/InterVariable.ttf")
                : "qrc:/qt/qml/CelestinaStyle/fonts/InterVariable.ttf"
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
    readonly property real sectionLetterSpacing: 1.4

    readonly property int weightRegular: Font.Normal    // 400 — body
    readonly property int weightMedium: Font.Medium     // 500 — kept until components settle to 400/600 (S4)
    readonly property int weightDemiBold: Font.DemiBold // 600 — titles

    // ── Radius scale ───────────────────────────────────────────────────────────
    // One UI's generous rounding: radius scales down with element size.
    // radiusButton/radiusInput are ready for CelestinaButton/TextField to adopt in
    // S4 (the button-emphasis + input-anatomy work); S1 leaves them defined.
    readonly property int radiusSm: 12       // controls, chips, glyph tiles, rows, banners
    readonly property int radiusXs: 3        // selection marquee / tiny indicators
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
    readonly property int controlHeightXs: 30
    readonly property int controlHeightSm: 34
    readonly property int controlHeight: 38
    readonly property int controlHeightLg: 42
    readonly property int controlHeightXl: 52
    readonly property int rowHeight: 54
    readonly property int rowHeightLg: 66
    readonly property int glyphTile: 34
    readonly property int glyphTileLg: 56
    readonly property int iconSm: 18
    readonly property int iconMd: 19
    readonly property int borderHairline: 1
    readonly property int borderFocus: 2
    readonly property real disabledOpacity: 0.5
    readonly property real disabledContentOpacity: 0.55
    readonly property real unavailableContentOpacity: 0.4
    readonly property real missingContentOpacity: 0.45
    readonly property real draggedContentOpacity: 0.9
    readonly property real mutedContentOpacity: 0.8
    readonly property real decorationOpacitySoft: 0.7
    readonly property real decorationOpacityStrong: 0.8

    // ── Motion ─────────────────────────────────────────────────────────────────
    // Host-controlled accessibility input. Consumers must bind this once from
    // their settings/platform adapter; every new animation provides a static
    // or fade-only route when it is true.
    property bool reducedMotion: false
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
    // Backdrop-blur knobs for GlassSurface (colours live in the scheme). The 8.5
    // recipe pairs blur with a *slight desaturation* of the backdrop (negative
    // saturation) — the earlier boost was half the recipe (audit §5.4). Keep
    // the four-pass pyramid at 32 and increase its reach with blurMultiplier;
    // this disperses text without stacking another effect pass.
    readonly property real glassBlur: 1.0
    readonly property int glassBlurMax: 32
    readonly property real glassBlurMultiplier: 3.0
    readonly property real glassSaturation: -0.03
    readonly property real glassSampleScale: 0.55
    readonly property int glassSampleMargin:
            Math.ceil(glassBlurMax * (1 + glassBlurMultiplier))
    // Fine noise dither over the blur — kills the banding the downsample pyramid
    // leaves behind (DESIGN §4/§6.5). Tiled texture at a low opacity.
    readonly property real glassNoiseOpacity: 0.025
    readonly property real glassEdgeWidth: 1.3
    readonly property real glassEdgeMidPosition: 0.35
    readonly property real glassEdgeLowPosition: 0.70
    readonly property real glassEdgeMidOpacity: 0.45
    readonly property real glassEdgeLowOpacity: 0.15

    // ── Elevation (scalars) ──────────────────────────────────────────────────
    // The L2 drop shadow (RectangularShadow) under floating layers. Soft, large
    // blur, low opacity, little offset — "must not suggest 3D depth" (§2). The
    // colour is `shadow` in the scheme; on near-black the shadow is a faint halo.
    readonly property int shadowBlur: 28
    readonly property int shadowSpread: 0
    readonly property int shadowOffsetY: 4

    // ── Component knobs (comp.*) ───────────────────────────────────────────────
    // Stable component anatomy belongs here; page placement and responsive
    // geometry stay with each screen. This is the QML equivalent of shared
    // component variables, without turning every coordinate into a token.
    readonly property int compButtonPaddingHorizontal: 14
    readonly property int compTextFieldPaddingHorizontal: 12
    readonly property int compSwitchTrackWidth: 44
    readonly property int compSwitchTrackHeight: 26
    readonly property int compSwitchThumbSize: 20
    readonly property int compSwitchThumbInset: 3
    readonly property int compCheckboxIndicatorSize: 18
    readonly property int compLinearTrackHeight: 4
    readonly property int compSliderHandleSize: 15
    readonly property int compStatusIndicatorSize: 8
    readonly property int compDragIndicatorHeight: 2
    readonly property int compSelectionIndicatorWidth: 3
    readonly property int compSelectionIndicatorHeight: 18
    readonly property int compMenuWidth: 232
    readonly property int compMenuPadding: 6
    readonly property int compMenuMargins: 24
    // Shared inset/gap for chrome floating inside a rounded content surface.
    // Keeping these semantic avoids each screen rebuilding the same 12/8 map.
    readonly property int compFloatingInset: spaceMd
    readonly property int compFloatingGap: spaceSm
    // A discrete mouse-wheel notch advances two standard rows. Touchpads keep
    // their native pixel delta and do not consume this metric.
    readonly property int compWheelStep: rowHeight * 2

    // Compatibility entry point for controls that expose `icon.source`.
    // Resolution itself belongs to the closed Lucide catalogue.
    function fallbackIcon(name) {
        return CelestinaIcons.source(name, "file")
    }
}
