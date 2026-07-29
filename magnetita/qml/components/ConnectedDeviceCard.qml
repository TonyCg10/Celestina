import QtQuick
import org.celestina.magnetita 1.0

CelestinaSurface {
    id: root

    required property string deviceName
    required property string deviceType
    required property string stateText
    required property string mountPath
    required property string fingerprint
    required property string batteryText
    required property bool charging

    readonly property bool mounted: mountPath.length > 0
    readonly property string batteryPercent: {
        const match = batteryText.match(/([0-9]+)/)
        return match ? match[1] : "—"
    }
    readonly property string batteryDescription:
            root.batteryPercent === "—"
            ? "Batería sin datos"
            : "Batería al " + root.batteryPercent + " por ciento"
              + (root.charging ? ", cargando" : "")

    height: 116
    role: CelestinaSurface.Grouped

    Rectangle {
        id: deviceTile
        anchors.left: parent.left
        anchors.leftMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        width: CelestinaTheme.glyphTileLg
        height: width
        radius: CelestinaTheme.radiusButton
        color: CelestinaTheme.accentSoft
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.accentSoftBorder

        CelestinaIcon {
            anchors.centerIn: parent
            width: CelestinaTheme.iconMd + CelestinaTheme.spaceSm
            height: width
            name: "phone"
            fallbackName: "phone"
            tone: CelestinaIcon.Device
        }
    }

    Column {
        anchors.left: deviceTile.right
        anchors.leftMargin: 14
        anchors.right: batteryBadge.left
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        Row {
            spacing: 6

            Rectangle {
                width: CelestinaTheme.compStatusIndicatorSize
                height: width
                radius: height / 2
                color: root.stateText === "desconectado"
                       ? CelestinaTheme.textFaint : CelestinaTheme.success
                anchors.verticalCenter: parent.verticalCenter
            }

            Text {
                text: root.stateText.length > 0 ? root.stateText : "conectado"
                color: root.stateText === "desconectado"
                       ? CelestinaTheme.textMuted : CelestinaTheme.success
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
            }
        }

        Text {
            width: parent.width
            text: root.deviceName
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
            elide: Text.ElideRight
        }

        Text {
            width: parent.width
            text: root.deviceType.length > 0
                  ? root.deviceType + (root.mounted ? " · " + root.mountPath : "")
                  : root.mounted ? root.mountPath : "Red local"
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideMiddle
        }

        Text {
            visible: root.fingerprint.length > 0
            width: parent.width
            text: root.fingerprint
            color: CelestinaTheme.textFaint
            font.family: CelestinaTheme.monoFamily
            font.pixelSize: CelestinaTheme.fontMini
            elide: Text.ElideRight
        }
    }

    Rectangle {
        id: batteryBadge
        anchors.right: parent.right
        anchors.rightMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        width: 54
        height: width
        radius: height / 2
        color: CelestinaTheme.accentSoft
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.accentSoftBorder
        Accessible.role: Accessible.StaticText
        Accessible.name: root.batteryDescription

        Row {
            anchors.centerIn: parent
            spacing: root.charging ? CelestinaTheme.spaceXs / 2 : 0
            Accessible.ignored: true

            CelestinaIcon {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.charging
                width: CelestinaTheme.iconSm - CelestinaTheme.spaceXs
                height: width
                name: "battery-charging"
                fallbackName: "battery-charging"
                tone: CelestinaIcon.Primary
                Accessible.ignored: true
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.batteryPercent
                      + (root.batteryPercent === "—" ? "" : "%")
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: root.charging ? CelestinaTheme.fontCaption
                                              : CelestinaTheme.fontBody
                font.weight: CelestinaTheme.weightDemiBold
                Accessible.ignored: true
            }
        }
    }
}
