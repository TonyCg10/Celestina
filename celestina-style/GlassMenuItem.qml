import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl

// ─── GlassMenuItem ────────────────────────────────────────────────────────────
// A MenuItem styled for GlassContextMenu: optional leading icon, `current`
// highlight, and the suite's hover/disabled treatment.
// ──────────────────────────────────────────────────────────────────────────────
MenuItem {
    id: control

    property bool current: false

    implicitWidth: CelestinaTheme.compMenuWidth - CelestinaTheme.compMenuPadding * 2
    implicitHeight: CelestinaTheme.controlHeight
    leftPadding: CelestinaTheme.spaceMd
    rightPadding: CelestinaTheme.spaceMd
    topPadding: CelestinaTheme.spaceSm
    bottomPadding: CelestinaTheme.spaceSm

    contentItem: Item {
        IconImage {
            id: menuIcon
            width: CelestinaTheme.iconSm
            height: CelestinaTheme.iconSm
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            visible: control.icon.name.length > 0
                     || control.icon.source.toString().length > 0
            name: control.icon.name
            source: control.icon.source
            // No colour: a tint repaints every pixel of a themed icon, which
            // flattened Qogir's folder, trash and star into white blobs. The
            // icon theme is trusted to fit a dark surface — the same call the
            // content views make — and "disabled" is carried by opacity alone.
            opacity: control.enabled ? 1 : CelestinaTheme.disabledContentOpacity
        }

        Text {
            anchors.left: menuIcon.visible ? menuIcon.right : parent.left
            anchors.leftMargin: menuIcon.visible ? CelestinaTheme.spaceSm : 0
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            text: control.text
            color: control.current
                   ? CelestinaTheme.accent
                   : control.enabled
                     ? CelestinaTheme.text
                     : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
            opacity: control.enabled ? 1 : CelestinaTheme.disabledContentOpacity
        }
    }

    background: Rectangle {
        radius: CelestinaTheme.radiusSm
        color: control.highlighted || control.current
               ? CelestinaTheme.surfaceHover
               : CelestinaTheme.clear

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.motionFast
            }
        }
    }
}
