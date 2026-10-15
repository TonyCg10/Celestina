// The shared visual heading for every PANEL-1 contextual surface.
//
// Real Qt menus and focused overlays keep different lifecycle owners. They do
// share this presentation: one denser section, a semantic icon, a title and
// status column, and a bounded trailing action column supplied by the surface.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: root

    required property BackdropInk ink
    required property string title
    property string subtitle: ""
    property string iconName: ""
    property string fallbackIcon: iconName
    property string trailingIconName: ""
    property string trailingText: ""
    property bool compact: false
    default property alias actions: actionRow.data

    implicitHeight: CelestinaTheme.rowHeight + CelestinaTheme.borderFocus
    objectName: "celestina-menu-header"
    Accessible.role: Accessible.StaticText
    Accessible.name: root.subtitle.length > 0
                     ? qsTr("%1, %2").arg(root.title).arg(root.subtitle)
                     : root.title

    MenuSection {
        ink: root.ink
    }

    CelestinaIcon {
        id: leadingIcon

        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceLg
        anchors.verticalCenter: parent.verticalCenter
        width: CelestinaTheme.iconMd
        height: width
        visible: root.iconName.length > 0
        name: root.iconName
        fallbackName: root.fallbackIcon
        tintOverride: root.ink.primary
        Accessible.ignored: true
    }

    Row {
        id: trailingRow

        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceLg
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceSm

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.trailingText.length > 0
            text: root.trailingText
            textFormat: Text.PlainText
            color: root.ink.faint
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontMini
            font.weight: CelestinaTheme.weightDemiBold
        }

        CelestinaIcon {
            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.iconSm
            height: width
            visible: root.trailingIconName.length > 0
            name: root.trailingIconName
            fallbackName: root.trailingIconName
            tintOverride: root.ink.muted
            Accessible.ignored: true
        }

        Row {
            id: actionRow

            anchors.verticalCenter: parent.verticalCenter
            spacing: CelestinaTheme.spaceXs
        }
    }

    Column {
        anchors.left: leadingIcon.visible ? leadingIcon.right : parent.left
        anchors.leftMargin: leadingIcon.visible ? CelestinaTheme.spaceMd
                                                : CelestinaTheme.spaceXl
        anchors.right: trailingRow.left
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceXs

        Text {
            width: parent.width
            text: root.title
            textFormat: Text.PlainText
            color: root.ink.primary
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: root.compact ? CelestinaTheme.fontRowTitle
                                         : CelestinaTheme.fontTitle
            font.weight: CelestinaTheme.weightDemiBold
            elide: Text.ElideRight
        }

        Text {
            width: parent.width
            visible: root.subtitle.length > 0
            text: root.subtitle
            textFormat: Text.PlainText
            color: root.ink.muted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: root.compact ? CelestinaTheme.fontMini
                                         : CelestinaTheme.fontCaption
            elide: Text.ElideRight
        }
    }
}
