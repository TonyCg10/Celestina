import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import org.celestina.siderita 1.0

MenuItem {
    id: control

    property bool current: false

    implicitWidth: CelestinaTheme.menuWidth - CelestinaTheme.menuPadding * 2
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
            color: control.enabled
                   ? CelestinaTheme.text
                   : CelestinaTheme.textMuted
            opacity: control.enabled ? 1 : 0.55
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
            opacity: control.enabled ? 1 : 0.55
        }
    }

    background: Rectangle {
        radius: CelestinaTheme.radiusXs
        color: control.highlighted || control.current
               ? CelestinaTheme.surfaceHover
               : "transparent"

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.motionFast
            }
        }
    }
}
