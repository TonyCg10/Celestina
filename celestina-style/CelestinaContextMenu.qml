import QtQuick

// ─── CelestinaContextMenu ─────────────────────────────────────────────────────
// Floating frosted-glass context menu with auto-flip positioning and fade
// animation.  Port of the iced build_context_menu overlay.
//
// Usage:
//   CelestinaContextMenu {
//       targetX: mouseX;  targetY: mouseY
//       windowWidth: root.width;  windowHeight: root.height
//       visible: showMenu
//
//       Column {
//           CelestinaButton { text: "📄  Nuevo Archivo"; onClicked: ... }
//           CelestinaButton { text: "📁  Nueva Carpeta"; onClicked: ... }
//       }
//   }
// ──────────────────────────────────────────────────────────────────────────────

Item {
    id: root

    // ── Public API ────────────────────────────────────────────────────────────
    default property alias menuContent: contentLoader.data

    property real targetX: 0          // Cursor X when menu was triggered
    property real targetY: 0          // Cursor Y when menu was triggered
    property real windowWidth: 800    // Parent window width (for flip calc)
    property real windowHeight: 600   // Parent window height

    property real menuWidth:  360
    property real estimatedHeight: 400

    // ── Auto-flip positioning (port of mod.rs L205-L211) ──────────────────────
    readonly property bool flipX: (targetX + menuWidth) > windowWidth
    readonly property bool flipY: (targetY + estimatedHeight) > windowHeight

    readonly property real posX: flipX ? (targetX - menuWidth) : targetX
    readonly property real posY: flipY ? (targetY - estimatedHeight) : targetY

    // Fill entire window for backdrop click
    anchors.fill: parent

    // ── Backdrop (click to close) ─────────────────────────────────────────────
    MouseArea {
        anchors.fill: parent
        onClicked: root.visible = false
    }

    // ── Glass Panel ───────────────────────────────────────────────────────────
    CelestinaGlassPanel {
        id: menuPanel
        x: Math.max(0, root.posX)
        y: Math.max(0, root.posY)
        width: root.menuWidth
        height: contentLoader.childrenRect.height + padding * 2

        glassBackground: CelestinaTheme.surfaceGlass
        glassBorder: CelestinaTheme.highlightMed
        glowColor: CelestinaTheme.glowIris
        padding: CelestinaTheme.spacingNormal

        // ── Fade-in animation ─────────────────────────────────────────────────
        opacity: root.visible ? 1.0 : 0.0
        Behavior on opacity {
            NumberAnimation {
                duration: CelestinaTheme.animFast
                easing.type: Easing.OutCubic
            }
        }

        // ── Scale-in animation ────────────────────────────────────────────────
        scale: root.visible ? 1.0 : 0.95
        transformOrigin: root.flipY
            ? (root.flipX ? Item.BottomRight : Item.BottomLeft)
            : (root.flipX ? Item.TopRight    : Item.TopLeft)
        Behavior on scale {
            NumberAnimation {
                duration: CelestinaTheme.animNormal
                easing.type: Easing.OutCubic
            }
        }

        // ── Menu Content ──────────────────────────────────────────────────────
        Item {
            id: contentLoader
            width: parent.width
            height: childrenRect.height
        }
    }
}
