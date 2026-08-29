// The ONE elevation shadow of this shell (SIMPLE-1, 2026-08-28).
//
// Two analytic layers, the anatomy macOS gives its control centre: a tight
// CONTACT shadow that seats the card, and a wide AMBIENT wash so faint it
// reads as depth rather than as a dark frame, whatever the wallpaper's
// luminance. One layer can never do both jobs — strong enough to seat a
// card over white it smokes the whole backdrop, faint enough to vanish on
// black it never seats anything. The author compared both states against
// macOS screenshots on a light and a dark desktop to land these tokens.
//
// Every consumer paints shade through this file. A second shadow
// implementation is how the shell shipped one calibrated surface and one
// smoking one at the same time; do not add another.
//
// Fill it against the rectangle that should cast — it paints OUTSIDE those
// bounds (each layer's canvas is twice its blur: at one blur-length the
// gaussian tail is still visible and cuts a hard rectangle on flat
// wallpapers), so no ancestor of the shadow may clip.
import QtQuick
import QtQuick.Effects

Item {
    id: root

    // The cast rectangle's corner radius.
    property real radius: CelestinaTheme.radiusMd

    RectangularShadow {
        readonly property real halo: CelestinaTheme.shadowAmbientBlur * 2
                                     + CelestinaTheme.shadowAmbientOffsetY

        anchors.fill: parent
        anchors.margins: -halo
        radius: root.radius
        blur: CelestinaTheme.shadowAmbientBlur
        spread: CelestinaTheme.shadowAmbientSpread - halo
        offset.y: CelestinaTheme.shadowAmbientOffsetY
        color: CelestinaTheme.shadowAmbient
    }

    RectangularShadow {
        readonly property real halo: CelestinaTheme.shadowContactBlur * 2

        anchors.fill: parent
        anchors.margins: -halo
        radius: root.radius
        blur: CelestinaTheme.shadowContactBlur
        spread: -halo
        offset.y: CelestinaTheme.shadowContactOffsetY
        color: CelestinaTheme.shadowContact
    }
}
