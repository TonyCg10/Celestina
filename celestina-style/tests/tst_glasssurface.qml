import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

TestCase {
    id: testCase

    name: "GlassSurface"
    when: testWindow.visible

    Window {
        id: testWindow

        width: 360
        height: 240
        visible: true

        Rectangle {
            id: backdrop

            anchors.fill: parent
            color: CelestinaTheme.canvas
        }

        GlassSurface {
            id: capturedGlass

            width: 120
            height: 80
            backdropSource: backdrop
        }

        GlassSurface {
            id: externalGlass

            x: 140
            width: 120
            height: 80
            backdropMode: GlassSurface.ExternalBackdrop
            externalBackdropReady: true
            materialTint: CelestinaTheme.glassHighlight
            materialOpacity: CelestinaTheme.decorationOpacitySoft / 2
        }

        GlassSurface {
            id: contentGlass

            y: 100
            width: 120
            height: 80
            backdropMode: GlassSurface.ExternalBackdrop
            externalBackdropReady: true
            captureEnabled: false
            materialRole: GlassSurface.ContentSurface
        }

        GlassSurface {
            id: contextualGlass

            x: 140
            y: 100
            width: 120
            height: 80
            backdropMode: GlassSurface.ExternalBackdrop
            externalBackdropReady: true
            captureEnabled: false
            materialRole: GlassSurface.ContextualVeil
        }

        GlassSurface {
            id: silhouetteGlass

            x: 280
            width: 72
            height: 80
            backdropMode: GlassSurface.ExternalBackdrop
            externalBackdropReady: true
            captureEnabled: false
            elevation: 0
            silhouettePath: "M 24 0 L 48 0 C 48 12 60 14 60 24 "
                            + "L 60 68 Q 60 80 48 80 L 24 80 "
                            + "Q 12 80 12 68 L 12 24 C 12 14 24 12 24 0 Z"
            silhouetteEdgePath: "M 48 0 C 48 12 60 14 60 24 "
                                + "L 60 68 Q 60 80 48 80 L 24 80 "
                                + "Q 12 80 12 68 L 12 24 C 12 14 24 12 24 0"
        }
    }

    function init() {
        capturedGlass.captureEnabled = true
        externalGlass.externalBackdropReady = true
        contentGlass.externalBackdropReady = true
        contextualGlass.externalBackdropReady = true
        silhouetteGlass.materialRole = GlassSurface.StandardMaterial
    }

    function test_in_scene_mode_owns_the_capture() {
        compare(capturedGlass.backdropMode, GlassSurface.InSceneCapture)
        verify(capturedGlass.captureActive)
        verify(capturedGlass.active)

        capturedGlass.captureEnabled = false
        verify(!capturedGlass.captureActive)
        verify(!capturedGlass.active)
    }

    function test_external_mode_never_starts_a_qml_capture() {
        compare(externalGlass.backdropSource, null)
        compare(externalGlass.backdropMode, GlassSurface.ExternalBackdrop)
        verify(externalGlass.active)
        verify(!externalGlass.captureActive)
        compare(externalGlass.materialTint, CelestinaTheme.glassHighlight)
        compare(externalGlass.materialOpacity,
                CelestinaTheme.decorationOpacitySoft / 2)
    }

    function test_external_mode_has_an_explicit_fallback_state() {
        externalGlass.externalBackdropReady = false
        verify(!externalGlass.active)
        verify(!externalGlass.captureActive)
    }

    function findByObjectName(item, name) {
        if (item.objectName === name)
            return item

        for (let index = 0; index < item.children.length; ++index) {
            const found = findByObjectName(item.children[index], name)
            if (found)
                return found
        }
        return null
    }

    function test_semantic_roles_preserve_the_default_and_separate_material_jobs() {
        compare(externalGlass.materialRole, GlassSurface.StandardMaterial)
        compare(externalGlass.materialStrength, 1)

        compare(contentGlass.materialRole, GlassSurface.ContentSurface)
        compare(contentGlass.materialTint, CelestinaTheme.canvas)
        compare(contentGlass.materialStrength,
                CelestinaTheme.glassContentSurfaceStrength)

        compare(contextualGlass.materialRole, GlassSurface.ContextualVeil)
        compare(contextualGlass.materialTint, CelestinaTheme.glassHighlight)
        compare(contextualGlass.materialStrength,
                CelestinaTheme.glassContextualVeilStrength)
        verify(contextualGlass.materialStrength < contentGlass.materialStrength)
        verify(contentGlass.materialEdgesVisible)
        verify(!contextualGlass.materialEdgesVisible)

        compare(contentGlass.captureActive, false)
        compare(contextualGlass.captureActive, false)
        compare(contentGlass.elevation, 0)
        compare(contextualGlass.elevation, 0)
        verify(!findByObjectName(contentGlass, "celestina-glass-shadow").visible)
        verify(!findByObjectName(contextualGlass, "celestina-glass-shadow").visible)

        const contentTint = findByObjectName(
            contentGlass, "celestina-glass-material-tint")
        const contextualTint = findByObjectName(
            contextualGlass, "celestina-glass-material-tint")
        const contextualNoise = findByObjectName(
            contextualGlass, "celestina-glass-noise")
        verify(contentTint)
        verify(contextualTint)
        verify(contextualNoise)
        verify(contextualNoise.visible)
        compare(contentTint.opacity,
                CelestinaTheme.glassContentSurfaceStrength)
        compare(contextualTint.opacity,
                CelestinaTheme.glassContextualVeilStrength)
        compare(contextualNoise.opacity,
                CelestinaTheme.glassNoiseOpacity
                * CelestinaTheme.glassContextualVeilStrength)
        verify(findByObjectName(
                   contentGlass, "celestina-glass-outline").visible)
        verify(findByObjectName(
                   contentGlass, "celestina-glass-lit-edge").visible)
        verify(!findByObjectName(
                   contextualGlass, "celestina-glass-outline").visible)
        verify(!findByObjectName(
                   contextualGlass, "celestina-glass-lit-edge").visible)
    }

    function test_silhouette_is_opt_in_and_keeps_the_semantic_material() {
        compare(capturedGlass.silhouettePath, "")
        verify(!capturedGlass.usesSilhouette)
        verify(silhouetteGlass.usesSilhouette)
        compare(silhouetteGlass.effectiveSilhouetteEdgePath,
                silhouetteGlass.silhouetteEdgePath)
        verify(findByObjectName(
                   silhouetteGlass,
                   "celestina-glass-silhouette-base").visible)
        const silhouetteTint = findByObjectName(
            silhouetteGlass, "celestina-glass-silhouette-material-tint")
        verify(silhouetteTint.visible)
        compare(silhouetteTint.opacity, silhouetteGlass.materialStrength)
        verify(findByObjectName(
                   silhouetteGlass,
                   "celestina-glass-silhouette-outline").visible)
        verify(findByObjectName(
                   silhouetteGlass,
                   "celestina-glass-silhouette-lit-edge").visible)
        verify(!findByObjectName(
                   silhouetteGlass,
                   "celestina-glass-outline").visible)
        verify(!findByObjectName(
                   silhouetteGlass,
                   "celestina-glass-lit-edge").visible)
        verify(!findByObjectName(
                   silhouetteGlass,
                   "celestina-glass-shadow").visible)

        silhouetteGlass.materialRole = GlassSurface.ContextualVeil
        verify(!silhouetteGlass.materialEdgesVisible)
        verify(silhouetteTint.visible)
        compare(silhouetteTint.opacity,
                CelestinaTheme.glassContextualVeilStrength)
        verify(!findByObjectName(
                   silhouetteGlass,
                   "celestina-glass-silhouette-outline").visible)
        verify(!findByObjectName(
                   silhouetteGlass,
                   "celestina-glass-silhouette-lit-edge").visible)
    }
}
