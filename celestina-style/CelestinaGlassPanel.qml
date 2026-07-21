import QtQuick
import QtQuick.Effects

// ─── CelestinaGlassPanel ─────────────────────────────────────────────────────
// A frosted-glass container with real Gaussian blur, luminous border, and
// colored glow shadow.  Drop any content inside as children.
//
// Usage:
//   CelestinaGlassPanel {
//       width: 300; height: 200
//       Text { text: "Hello"; color: CelestinaTheme.text }
//   }
// ──────────────────────────────────────────────────────────────────────────────

Item {
    id: root

    // ── Public API ────────────────────────────────────────────────────────────
    default property alias contentData: contentContainer.data

    property real   glassRadius:     CelestinaTheme.radiusLarge
    property color  glassBackground: CelestinaTheme.surfaceGlass
    property color  glassBorder:     CelestinaTheme.highlightMed
    property real   glassBorderWidth: CelestinaTheme.borderThin
    property real   blurAmount:      CelestinaTheme.glassBlur
    property real   saturationAmount: CelestinaTheme.glassSaturation
    property bool   glowEnabled:     true
    property color  glowColor:       CelestinaTheme.glowIris
    property real   padding:         CelestinaTheme.spacingNormal

    // ── Glow Shadow Layer ─────────────────────────────────────────────────────
    Rectangle {
        id: glowShadow
        anchors.fill: glassRect
        anchors.margins: -4
        radius: glassRect.radius + 4
        color: "transparent"
        visible: root.glowEnabled

        layer.enabled: root.glowEnabled
        layer.effect: MultiEffect {
            blurEnabled: true
            blur: 0.4
            colorization: 1.0
            colorizationColor: root.glowColor
        }

        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: root.glowColor
        }
    }

    // ── Glass Background ──────────────────────────────────────────────────────
    Rectangle {
        id: glassRect
        anchors.fill: parent
        radius: root.glassRadius
        color: root.glassBackground
        clip: true

        border.color: root.glassBorder
        border.width: root.glassBorderWidth

        // Blur the content behind this panel
        layer.enabled: true
        layer.effect: MultiEffect {
            blurEnabled: true
            blur: root.blurAmount
            saturation: root.saturationAmount
        }
    }

    // ── Content Container ─────────────────────────────────────────────────────
    Item {
        id: contentContainer
        anchors.fill: parent
        anchors.margins: root.padding
    }
}
