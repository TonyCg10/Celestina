import QtQuick
import QtQuick.Controls

// ─── CelestinaButton ──────────────────────────────────────────────────────────
// Ghost-style button with a smooth hover-color transition and a press
// micro-scale. Uses the suite's semantic tokens.
//
// Usage:
//   CelestinaButton {
//       text: "Paste"
//       onClicked: root.paste()
//   }
// ──────────────────────────────────────────────────────────────────────────────
AbstractButton {
    id: root

    property color textColor: CelestinaTheme.text
    property color hoverBackground: CelestinaTheme.surfaceHover
    property int fontSize: CelestinaTheme.fontLabel
    property string fontFamily: CelestinaTheme.sansFamily
    property int radius: CelestinaTheme.radiusSm

    implicitWidth: Math.max(label.implicitWidth + 24, 60)
    implicitHeight: label.implicitHeight + 12

    // ── Animated background ───────────────────────────────────────────────────
    background: Rectangle {
        id: bg
        radius: root.radius
        color: root.hovered || root.pressed
            ? root.hoverBackground
            : "transparent"

        Behavior on color {
            ColorAnimation { duration: CelestinaTheme.motionFast }
        }

        // Subtle border on hover
        border.color: root.hovered
            ? CelestinaTheme.borderStrong
            : "transparent"
        border.width: 1

        Behavior on border.color {
            ColorAnimation { duration: CelestinaTheme.motionNormal }
        }
    }

    // ── Press micro-animation ─────────────────────────────────────────────────
    scale: root.pressed ? 0.97 : 1.0
    Behavior on scale {
        NumberAnimation {
            duration: CelestinaTheme.motionFast
            easing.type: Easing.OutCubic
        }
    }

    // ── Label ─────────────────────────────────────────────────────────────────
    contentItem: Text {
        id: label
        text: root.text
        color: root.textColor
        font.family: root.fontFamily
        font.pixelSize: root.fontSize
        verticalAlignment: Text.AlignVCenter
        leftPadding: 8
        rightPadding: 8
    }
}
