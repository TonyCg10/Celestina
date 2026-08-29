import QtQuick
import QtQuick.Effects
import QtQuick.Shapes

// ─── GlassSurface ─────────────────────────────────────────────────────────────
// Frosted-glass surface over either an in-scene item or an external backdrop
// already supplied by the compositor. InSceneCapture bounds ShaderEffectSource
// to this surface. ExternalBackdrop never starts a QML capture — another
// Wayland client is not part of this scene — and renders the same material over
// the host-provided blur or fallback instead.
//
// Recipe (One UI 8.5, DESIGN §6.5): bounded capture → pyramid blur → *slight
// desaturation* + tint/dim → a thin dark outline for definition → the lit
// top-edge glow. ContextualVeil deliberately stops before those edge layers:
// it is a carrier, not another outlined card. `elevation > 0` adds the L2 drop
// shadow (a floating layer stops pasting and starts floating). The shadow lives
// outside the clipped body, so the root itself does not clip.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    enum Density {
        Regular,
        Strong
    }

    enum BackdropMode {
        InSceneCapture,
        ExternalBackdrop
    }

    enum MaterialRole {
        StandardMaterial,
        ContentSurface,
        ContextualVeil
    }

    // Required by InSceneCapture and deliberately unused by ExternalBackdrop.
    // Keeping null as a truthful fallback lets one public type cover both
    // backends without forcing a compositor host to inject a fake scene item.
    property Item backdropSource: null
    property int backdropMode: GlassSurface.InSceneCapture
    property bool externalBackdropReady: false
    property bool captureEnabled: true
    // One-shot snapshot on show (false) vs continuous re-capture while shown
    // (true). Live costs a small per-frame render, but it is what makes the
    // surface read as glass rather than as a frozen picture of the moment it
    // opened, so anything the user can scroll, hover or drag under wants it.
    property bool liveCapture: false
    property real cornerRadius: CelestinaTheme.radiusMd
    // Optional vector silhouette for ExternalBackdrop surfaces that grow into
    // a screen edge. Empty preserves the historical rounded rectangle
    // byte-for-byte. The host still owns compositor geometry; this path changes
    // only how this component paints its canonical semantic material.
    property string silhouettePath: ""
    // A screen-edge silhouette can omit its upper mouth from the painted
    // strokes so the pane appears to continue beyond the viewport instead of
    // wearing a hairline cap at y=0. Empty reuses the complete silhouette.
    property string silhouetteEdgePath: ""
    readonly property bool usesSilhouette: silhouettePath.length > 0
    readonly property string effectiveSilhouetteEdgePath:
            silhouetteEdgePath.length > 0 ? silhouetteEdgePath : silhouettePath
    property int sampleMargin: CelestinaTheme.glassSampleMargin
    property real sampleScale: CelestinaTheme.glassSampleScale
    // Elevation level (DESIGN §6.4): 0 = flush (grouped card, separation by
    // grouping), 2 = floating (menu, tooltip, pill, toast) → L2 drop shadow.
    // Modals (L3) use a scrim behind, never a shadow, so they stay at 0.
    property int elevation: 0
    property int density: GlassSurface.Regular
    // The default is intentionally pixel-compatible with the pre-role public
    // material. ContentSurface and ContextualVeil are opt-in semantic jobs;
    // they never change capture, compositor ownership, geometry or elevation.
    property int materialRole: GlassSurface.StandardMaterial
    default property alias contentData: foreground.data

    // Consumers may supply a semantic, state-derived tint while the component
    // remains the sole owner of material ordering, noise, outline and lit edge.
    property color materialTint:
            materialRole === GlassSurface.ContentSurface
            ? CelestinaTheme.canvas
            : materialRole === GlassSurface.ContextualVeil
              ? CelestinaTheme.glassHighlight
              : density === GlassSurface.Strong
                ? CelestinaTheme.glassTintStrong
                : CelestinaTheme.glassTint
    property real materialOpacity: 1
    readonly property real materialStrength:
            materialRole === GlassSurface.ContentSurface
            ? CelestinaTheme.glassContentSurfaceStrength
            : materialRole === GlassSurface.ContextualVeil
              ? CelestinaTheme.glassContextualVeilStrength
              : 1
    // A contextual carrier must not acquire a second card boundary. At its low
    // material strength even a hairline outline reads as an exterior shadow,
    // especially around a narrow connector. Dense and default glass retain the
    // complete dark-outline plus lit-edge recipe.
    readonly property bool materialEdgesVisible:
            materialRole !== GlassSurface.ContextualVeil

    readonly property bool captureActive:
            backdropMode === GlassSurface.InSceneCapture
            && captureEnabled
            && backdropSource !== null
            && !usesSilhouette
            && width > 0
            && height > 0
    readonly property bool active:
            backdropMode === GlassSurface.ExternalBackdrop
            ? externalBackdropReady && width > 0 && height > 0
            : captureActive

    function refreshBackdrop() {
        if (!captureActive)
            return

        const point = sampleLayer.mapToItem(backdropSource, 0, 0)
        capture.sourceRect = Qt.rect(point.x, point.y,
                                     sampleLayer.width, sampleLayer.height)
        capture.scheduleUpdate()
    }

    onCaptureActiveChanged: {
        if (captureActive)
            refreshBackdrop()
        else
            capture.sourceRect = Qt.rect(0, 0, 0, 0)
    }
    onBackdropSourceChanged: refreshBackdrop()

    // The sampled region has to follow the surface. Its size can still change
    // after it is shown — a menu grows as its items decide to be visible — and
    // a sourceRect left at the old size shows a stretched, wrong-looking region
    // instead of what is actually behind. A consumer that *moves* the surface
    // (a popup being positioned) re-arms this by calling refreshBackdrop().
    onWidthChanged: refreshBackdrop()
    onHeightChanged: refreshBackdrop()

    // Live capture updates the sampled pixels, but not the coordinate mapping.
    // When the injected source itself moves or resizes (for example, Siderita's
    // viewport while its heading expands), refresh only for that short geometry
    // transition. This is event-driven and leaves the GUI thread idle at rest.
    Connections {
        target: root.backdropSource
        enabled: root.captureActive

        function onXChanged() { root.refreshBackdrop() }
        function onYChanged() { root.refreshBackdrop() }
        function onWidthChanged() { root.refreshBackdrop() }
        function onHeightChanged() { root.refreshBackdrop() }
    }

    // A moving surface (a popup being positioned) still re-arms the sample from
    // its consumer, on the event that moved it. An earlier always-on
    // FrameAnimation re-sampled on the GUI thread even at idle; geometry signals
    // and the existing popup hooks cover both directions without frame polling.

    // L2 drop shadow, behind the body and outside its clip. RectangularShadow is
    // an analytic SDF (Qt 6.9+) — far cheaper than a MultiEffect shadow and it
    // extends past the rect, which is why the root must not clip.
    CelestinaShadow {
        objectName: "celestina-glass-shadow"
        anchors.fill: body
        // A shaped pane has no analytic rectangular shadow. Current shell edge
        // surfaces are deliberately flush (elevation 0); a later reusable
        // shaped-elevation job needs its own bounded vector shadow contract.
        visible: root.elevation > 0 && !root.usesSilhouette
        radius: root.cornerRadius
    }

    // The glass itself, clipped to the rounded rectangle. Everything visible
    // lives here so the shadow above can spill while the content cannot.
    Item {
        id: body
        anchors.fill: parent
        clip: !root.usesSilhouette

        Rectangle {
            anchors.fill: parent
            visible: !root.usesSilhouette
            radius: root.cornerRadius
            color: root.backdropMode === GlassSurface.ExternalBackdrop
                   ? CelestinaTheme.clear
                   : CelestinaTheme.surfaceStrong
        }

        Item {
            id: sampleLayer
            x: -root.sampleMargin
            y: -root.sampleMargin
            width: root.width + root.sampleMargin * 2
            height: root.height + root.sampleMargin * 2
            visible: root.captureActive

            ShaderEffectSource {
                id: capture
                anchors.fill: parent
                sourceItem: root.captureActive ? root.backdropSource : null
                sourceRect: Qt.rect(0, 0, 0, 0)
                textureSize: Qt.size(
                    Math.max(1, Math.ceil(width * root.sampleScale)),
                    Math.max(1, Math.ceil(height * root.sampleScale)))
                live: root.liveCapture
                recursive: false
                hideSource: false
                smooth: true
                visible: false
            }

            Item {
                id: roundedMask
                anchors.fill: parent
                visible: false
                layer.enabled: true

                Rectangle {
                    x: root.sampleMargin
                    y: root.sampleMargin
                    width: root.width
                    height: root.height
                    radius: root.usesSilhouette ? 0 : root.cornerRadius
                    color: CelestinaTheme.opaqueMask
                }
            }

            MultiEffect {
                anchors.fill: parent
                source: capture
                visible: root.captureActive
                blurEnabled: true
                blur: CelestinaTheme.glassBlur
                blurMax: CelestinaTheme.glassBlurMax
                blurMultiplier: CelestinaTheme.glassBlurMultiplier
                // Slight desaturation of the backdrop — the 8.5 recipe, not the
                // earlier saturation boost (a negative value here desaturates).
                saturation: CelestinaTheme.glassSaturation
                autoPaddingEnabled: false
                maskEnabled: true
                maskSource: roundedMask
            }
        }

        Rectangle {
            objectName: "celestina-glass-material-tint"
            anchors.fill: parent
            visible: !root.usesSilhouette
            radius: root.cornerRadius
            color: root.active ? root.materialTint : CelestinaTheme.surfaceStrong
            opacity: root.active
                     ? root.materialOpacity * root.materialStrength
                     : 1
        }

        // Fine noise dither over the blur — breaks the banding the downsample
        // pyramid leaves. Tiled at a low opacity; the body clip keeps it inside.
        Image {
            objectName: "celestina-glass-noise"
            anchors.fill: parent
            visible: root.active && !root.usesSilhouette
            source: Qt.resolvedUrl(".").toString().startsWith("file:")
                    ? Qt.resolvedUrl("icons/glass-noise.png")
                    : "qrc:/qt/qml/CelestinaStyle/icons/glass-noise.png"
            fillMode: Image.Tile
            opacity: CelestinaTheme.glassNoiseOpacity * root.materialStrength
            smooth: false
        }

        Shape {
            objectName: "celestina-glass-silhouette-base"
            anchors.fill: parent
            visible: root.usesSilhouette
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeWidth: 0
                fillColor: root.backdropMode === GlassSurface.ExternalBackdrop
                           ? CelestinaTheme.clear
                           : CelestinaTheme.surfaceStrong
                PathSvg { path: root.silhouettePath }
            }
        }

        Shape {
            objectName: "celestina-glass-silhouette-material-tint"
            anchors.fill: parent
            visible: root.usesSilhouette
            opacity: root.active
                     ? root.materialOpacity * root.materialStrength
                     : 1
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeWidth: 0
                fillColor: root.active
                           ? root.materialTint
                           : CelestinaTheme.surfaceStrong
                PathSvg { path: root.silhouettePath }
            }
        }

        // A thin dark outline (dark outside) — gives the pane an edge against a
        // light backdrop where the lit glow alone would wash out. One UI's glass
        // wears both: a dark hairline and the lit top edge.
        Rectangle {
            objectName: "celestina-glass-outline"
            anchors.fill: parent
            radius: root.cornerRadius
            visible: root.materialEdgesVisible && !root.usesSilhouette
            color: CelestinaTheme.clear
            border.width: CelestinaTheme.borderHairline
            border.color: root.active
                          ? CelestinaTheme.multiplyAlpha(
                                CelestinaTheme.glassOutline,
                                root.materialStrength)
                          : CelestinaTheme.glassOutline
        }

        // The lit glass edge: a rounded-rect gradient stroke, brightest along the
        // top and fading to nothing at the bottom, so the pane catches light like
        // real glass instead of wearing a flat box border. A GPU Shape
        // (CurveRenderer) fills a ~1.3px ring — two rounded PathRectangles under
        // an odd-even fill — with a vertical gradient. Replaces the CPU Canvas,
        // which re-rastered the whole edge on every resize (DESIGN §6.5).
        Shape {
            objectName: "celestina-glass-lit-edge"
            anchors.fill: parent
            visible: root.active && root.materialEdgesVisible
                     && !root.usesSilhouette
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                fillRule: ShapePath.OddEvenFill
                strokeWidth: 0
                fillColor: CelestinaTheme.clear
                fillGradient: LinearGradient {
                    x1: 0
                    y1: 0
                    x2: 0
                    y2: root.height
                    GradientStop {
                        position: 0
                        color: CelestinaTheme.multiplyAlpha(
                                   CelestinaTheme.glassBorder,
                                   root.materialStrength)
                    }
                    GradientStop {
                        position: CelestinaTheme.glassEdgeMidPosition
                        color: CelestinaTheme.multiplyAlpha(
                                   CelestinaTheme.glassBorder,
                                   CelestinaTheme.glassEdgeMidOpacity
                                   * root.materialStrength)
                    }
                    GradientStop {
                        position: CelestinaTheme.glassEdgeLowPosition
                        color: CelestinaTheme.multiplyAlpha(
                                   CelestinaTheme.glassBorder,
                                   CelestinaTheme.glassEdgeLowOpacity
                                   * root.materialStrength)
                    }
                    GradientStop {
                        position: 1
                        color: CelestinaTheme.clear
                    }
                }
                PathRectangle {
                    x: 0
                    y: 0
                    width: root.width
                    height: root.height
                    radius: root.cornerRadius
                }
                PathRectangle {
                    x: CelestinaTheme.glassEdgeWidth
                    y: CelestinaTheme.glassEdgeWidth
                    width: root.width - CelestinaTheme.glassEdgeWidth * 2
                    height: root.height - CelestinaTheme.glassEdgeWidth * 2
                    radius: Math.max(0, root.cornerRadius
                                        - CelestinaTheme.glassEdgeWidth)
                }
            }
        }

        // No seam treatment rides the silhouette. The finite blur behind a
        // shaped surface is a pixel region whose boundary cannot be
        // antialiased; two attempts to cover it in paint — a stroke in the
        // material's tint, then the same band blurred into a shadow — both
        // read as a border the author rejected outright. What remains against
        // the staircase is geometry alone: the host samples the region at
        // curve-following density and tucks it one pixel inside the painted
        // silhouette.

        // The opt-in silhouette keeps the same dark definition and lit-glass
        // vocabulary. A generic path cannot derive a mathematically inset ring
        // without changing the host's geometry, so these strokes are clipped
        // with the material and remain entirely inside its finite silhouette.
        Shape {
            objectName: "celestina-glass-silhouette-outline"
            anchors.fill: parent
            visible: root.materialEdgesVisible && root.usesSilhouette
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeWidth: CelestinaTheme.borderHairline * 2
                strokeColor: root.active
                             ? CelestinaTheme.multiplyAlpha(
                                   CelestinaTheme.glassOutline,
                                   root.materialStrength)
                             : CelestinaTheme.glassOutline
                fillColor: CelestinaTheme.clear
                PathSvg { path: root.effectiveSilhouetteEdgePath }
            }
        }

        Shape {
            objectName: "celestina-glass-silhouette-lit-edge"
            anchors.fill: parent
            visible: root.active && root.materialEdgesVisible
                     && root.usesSilhouette
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                strokeWidth: CelestinaTheme.glassEdgeWidth * 2
                strokeColor: CelestinaTheme.multiplyAlpha(
                                 CelestinaTheme.glassBorder,
                                 CelestinaTheme.glassEdgeLowOpacity
                                 * root.materialStrength)
                fillColor: CelestinaTheme.clear
                PathSvg { path: root.effectiveSilhouetteEdgePath }
            }
        }

        Item {
            id: foreground
            anchors.fill: parent
        }
    }
}
