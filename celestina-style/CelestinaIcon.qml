import QtQuick
import QtQuick.Controls.impl

// One icon contract for native theme names and bundled Lucide fallbacks.
// Geometry remains consumer-owned; tone is semantic so app code never tints
// an icon with an arbitrary colour.
IconImage {
    id: icon

    enum Tone {
        Native,
        Primary,
        Secondary,
        Accent,
        OnAccent,
        Overlay,
        Favorite,
        Danger
    }

    property string fallbackName: ""
    property int tone: CelestinaIcon.Native

    width: CelestinaTheme.iconSm
    height: CelestinaTheme.iconSm
    source: fallbackName.length > 0
            ? CelestinaTheme.fallbackIcon(fallbackName)
            : ""
    sourceSize: Qt.size(width, height)
    color: {
        switch (tone) {
        case CelestinaIcon.Primary:
            return CelestinaTheme.text
        case CelestinaIcon.Secondary:
            return CelestinaTheme.textMuted
        case CelestinaIcon.Accent:
            return CelestinaTheme.accent
        case CelestinaIcon.OnAccent:
            return CelestinaTheme.accentInk
        case CelestinaIcon.Overlay:
            return CelestinaTheme.mediaScrimInk
        case CelestinaIcon.Favorite:
            return CelestinaTheme.favorite
        case CelestinaIcon.Danger:
            return CelestinaTheme.dangerFillInk
        default:
            return CelestinaTheme.clear
        }
    }
}
