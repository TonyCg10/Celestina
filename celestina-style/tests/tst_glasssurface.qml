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
    }

    function init() {
        capturedGlass.captureEnabled = true
        externalGlass.externalBackdropReady = true
        contentGlass.externalBackdropReady = true
        contextualGlass.externalBackdropReady = true
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
        compare(contentTint.opacity,
                CelestinaTheme.glassContentSurfaceStrength)
        compare(contextualTint.opacity,
                CelestinaTheme.glassContextualVeilStrength)
        compare(contextualNoise.opacity,
                CelestinaTheme.glassNoiseOpacity
                * CelestinaTheme.glassContextualVeilStrength)
    }
}
