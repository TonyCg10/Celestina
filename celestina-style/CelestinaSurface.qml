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

    readonly property real radius: role === CelestinaSurface.Canvas
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
