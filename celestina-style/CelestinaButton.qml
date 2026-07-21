import QtQuick
import QtQuick.Controls

// ─── CelestinaButton ──────────────────────────────────────────────────────────
// Ghost-style button with smooth hover color transition and micro-scale
// animation.  Port of celestina-ui's `ghost_button` style.
//
// Usage:
//   CelestinaButton {
//       text: "📋  Pegar"
//       onClicked: root.paste()
//   }
// ──────────────────────────────────────────────────────────────────────────────

AbstractButton {
    id: root

    property color textColor:       CelestinaTheme.text
    property color hoverBackground: Qt.rgba(
        CelestinaTheme.highlightMed.r,
        CelestinaTheme.highlightMed.g,
        CelestinaTheme.highlightMed.b,
        0.3
    )
    property int    fontSize:        CelestinaTheme.fontSmall
    property string fontFamily:      CelestinaTheme.fontFamily
    property int    radius:          CelestinaTheme.radiusMedium

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
            ColorAnimation { duration: CelestinaTheme.animFast }
        }

        // Subtle glow border on hover
        border.color: root.hovered
            ? CelestinaTheme.highlightHigh
            : "transparent"
        border.width: CelestinaTheme.borderThin

        Behavior on border.color {
            ColorAnimation { duration: CelestinaTheme.animNormal }
        }
    }

    // ── Press micro-animation ─────────────────────────────────────────────────
    scale: root.pressed ? 0.97 : 1.0
    Behavior on scale {
        NumberAnimation {
            duration: CelestinaTheme.animFast
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
