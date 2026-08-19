import QtQuick
import QtQuick.Controls

// Semantic container for every non-floating Celestina surface.
//
// Consumers choose the surface's meaning and geometry; the design system owns
// its fill, foreground pairing and shape. Floating and modal layers continue to
// use GlassSurface/GlassCard because they also need a backdrop source.
Pane {
    id: control

    enum Role {
        Canvas,
        Panel,
        Grouped,
        Content,
        Tonal,
        Elevated,
        Selected
    }

    property int role: CelestinaSurface.Grouped

    readonly property color ink: role === CelestinaSurface.Canvas
                                         ? CelestinaTheme.canvasInk
                                      : role === CelestinaSurface.Elevated
                                         ? CelestinaTheme.elevatedInk
                                         : CelestinaTheme.cardInk

    // A caller may set its own corner when the surface has to agree with
    // something outside the design system — the window's own rounding, for
    // instance. Below zero means "use the role's".
    property int radiusOverride: -1

    readonly property real radius: control.radiusOverride >= 0
                                          ? control.radiusOverride
                                       : role === CelestinaSurface.Canvas
                                          ? 0
                                        : role === CelestinaSurface.Content
                                          || role === CelestinaSurface.Tonal
                                          || role === CelestinaSurface.Selected
                                          ? CelestinaTheme.radiusMd
                                          : CelestinaTheme.radiusLg

    readonly property color fill: role === CelestinaSurface.Canvas
                                         ? CelestinaTheme.canvas
                                      : role === CelestinaSurface.Panel
                                         || role === CelestinaSurface.Tonal
                                         ? CelestinaTheme.card
                                      : role === CelestinaSurface.Elevated
                                         ? CelestinaTheme.elevated
                                      : role === CelestinaSurface.Selected
                                         ? CelestinaTheme.surfaceSelected
                                         : CelestinaTheme.card

    padding: 0

    background: Rectangle {
        radius: control.radius
        color: control.fill
        border.width: control.role === CelestinaSurface.Canvas
                      ? 0 : CelestinaTheme.borderHairline
        border.color: control.role === CelestinaSurface.Selected
                      ? CelestinaTheme.dividerStrong
                      : CelestinaTheme.divider
    }
}
