// One fixed light-ink palette for shell glass.
//
// Contextual carriers remain nearly transparent, while information-bearing
// sections and panel capsules use the canonical dark content material below
// this light foreground. The shell deliberately does not sample wallpaper or
// application pixels to switch foreground polarity.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

QtObject {
    id: root

    readonly property color neutral: CelestinaTheme.text

    // The outer contextual veil must stay very light. Dense content surfaces
    // use the dark tint below instead of turning the complete carrier opaque.
    readonly property color materialTint: CelestinaTheme.glassHighlight
    readonly property color contentMaterialTint: CelestinaTheme.canvas

    // Neutral hierarchy comes from type and placement. Shell text and glyphs
    // share one stable light foreground over the dark content material.
    readonly property color primary: root.neutral
    readonly property color muted: root.neutral
    readonly property color faint: root.neutral
    readonly property color accent: root.neutral
    readonly property color danger: root.neutral
    readonly property color warning: root.neutral
    readonly property color focus: root.neutral

    // Retain the established light-ink interaction layers.
    readonly property color divider: CelestinaTheme.divider
    readonly property color dividerStrong: CelestinaTheme.dividerStrong
    readonly property color controlFill: CelestinaTheme.controlFill
    readonly property color hoverFill: CelestinaTheme.surfaceHover
    readonly property color pressedFill: CelestinaTheme.surfaceStrong
    readonly property color selectedFill: CelestinaTheme.surfaceSelected
    readonly property color accentFill: CelestinaTheme.accentSoft
    readonly property color selectedRestFill: CelestinaTheme.badgeAccentFill
}
