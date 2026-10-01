pragma ComponentBehavior: Bound

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
// Recipe (Haze 1.6, the phone's glass; DESIGN §5.3): bounded capture at full
// resolution → blur of σ ≈ 12 px → Haze's grain at 0.15 → the canvas tint at
// 0.70. Nothing else: no desaturation, no outline, no lit edge. A role only
// scales the grain and the tint (`materialStrength`). `elevation > 0` adds the
// L2 drop shadow, which is the phone pill's `shadowElevation`. When the surface
// cannot blur it paints the opaque canvas, which is what Haze's
// `backgroundColor` does. The shadow lives outside the clipped body, so the
// root itself does not clip.
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
    // Retained public API; no stroke reads them since the outline and lit
    // edge were removed.
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
    // remains the sole owner of material ordering and grain.
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
                   : CelestinaTheme.glassFallback
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
                autoPaddingEnabled: false
                maskEnabled: true
                maskSource: roundedMask
            }
        }

        // Haze's grain, drawn over the blur and under the tint: its texture
        // tiled with alpha 0.15 (times the role's strength). The order is
        // Haze's — blur, noise, then tints — and it is what the phone shows.
        // The grain is masked to the same rounded corners as the tint and blur.
        Item {
            id: noiseMask
            anchors.fill: parent
            visible: false
            layer.enabled: true

            Rectangle {
                anchors.fill: parent
                radius: root.cornerRadius
                color: CelestinaTheme.opaqueMask
            }
        }

        Item {
            objectName: "celestina-glass-noise-mask"
            anchors.fill: parent
            visible: root.active && !root.usesSilhouette
            layer.enabled: true
            layer.effect: MultiEffect {
                maskEnabled: true
                maskSource: noiseMask
            }

            Image {
                objectName: "celestina-glass-noise"
                anchors.fill: parent
                visible: root.active && !root.usesSilhouette
                source: Qt.resolvedUrl(".").toString().startsWith("file:")
                        ? Qt.resolvedUrl("icons/haze-noise.png")
                        : "qrc:/qt/qml/CelestinaStyle/icons/haze-noise.png"
                fillMode: Image.Tile
                opacity: CelestinaTheme.glassNoiseOpacity * root.materialStrength
                smooth: false
            }
        }

        Rectangle {
            objectName: "celestina-glass-material-tint"
            anchors.fill: parent
            visible: !root.usesSilhouette
            radius: root.cornerRadius
            color: root.active ? root.materialTint : CelestinaTheme.glassFallback
            opacity: root.active
                     ? root.materialOpacity * root.materialStrength
                     : 1
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
                           : CelestinaTheme.glassFallback
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
                           : CelestinaTheme.glassFallback
                PathSvg { path: root.silhouettePath }
            }
        }

        Item {
            id: foreground
            anchors.fill: parent
        }
    }
}
