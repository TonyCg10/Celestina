import QtQuick
import QtQuick.Controls.impl
import QtQuick.Window

// One local-only Lucide contract. `name` remains a semantic compatibility API
// for existing callers, but it is never passed to QIcon: the catalogue resolves
// it to a vendored SVG, making the suite independent of the desktop icon theme.
Item {
    id: icon

    enum Tone {
        Native,
        Primary,
        Secondary,
        Accent,
        OnAccent,
        Overlay,
        Favorite,
        Danger,
        Folder,
        File,
        Symlink,
        Navigation,
        Device
    }

    property string name: ""
    property string fallbackName: ""
    property url source: ""
    // Rasterized at the screen's real pixel density, not at the item's
    // logical size. `IconImage` renders the vendored SVG through
    // `QSvgIconEngine` once, at exactly this many pixels; asking for the
    // logical size on any output above 1.0 scale handed the compositor a
    // pixmap smaller than the physical area it fills, which is what read as
    // pixelated on a fractionally-scaled monitor while an integer-scaled one
    // never showed it.
    property size sourceSize: Qt.size(
            rasterSide * icon.effectiveDevicePixelRatio,
            rasterSide * icon.effectiveDevicePixelRatio)
    property int tone: CelestinaIcon.Native
    // Per-item accents remain an explicit override. A transparent value means
    // “use the semantic tone”, keeping ordinary callers fully token-driven.
    property color tintOverride: CelestinaTheme.clear
    readonly property int rasterSide:
            Math.max(1, Math.round(Math.min(width, height)))
    // `Screen` is only valid once this item belongs to a window. Before that
    // — the first layout pass of any freshly constructed icon — it resolves
    // to a null attached object; 1.0 is the same density QML itself assumes
    // in that state, not a guess this file introduces.
    readonly property real effectiveDevicePixelRatio:
            icon.Screen ? icon.Screen.devicePixelRatio : 1.0
    readonly property url resolvedSource:
            source.toString().length > 0
            ? source : CelestinaIcons.source(name, fallbackName)
    readonly property color semanticTint: {
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
        case CelestinaIcon.Folder:
            return CelestinaTheme.glyphDirectory
        case CelestinaIcon.File:
            return CelestinaTheme.glyphFile
        case CelestinaIcon.Symlink:
            return CelestinaTheme.glyphSymlink
        case CelestinaIcon.Navigation:
            return CelestinaTheme.glyphNavigation
        case CelestinaIcon.Device:
            return CelestinaTheme.glyphDevice
        default:
            // "Native" now means the neutral suite tone; there is no native
            // theme bitmap left whose original colours need preserving.
            return CelestinaTheme.textMuted
        }
    }
    readonly property color tint: tintOverride.a > 0
                                  ? tintOverride : semanticTint

    width: CelestinaTheme.iconSm
    height: CelestinaTheme.iconSm

    IconImage {
        id: glyph
        anchors.fill: parent
        name: ""
        source: icon.resolvedSource
        sourceSize: icon.sourceSize
        fillMode: Image.PreserveAspectFit
        horizontalAlignment: Image.AlignHCenter
        verticalAlignment: Image.AlignVCenter
        smooth: true
        mipmap: true
        color: icon.tint
    }
}
