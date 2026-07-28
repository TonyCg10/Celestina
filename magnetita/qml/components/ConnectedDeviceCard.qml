import QtQuick
import org.celestina.magnetita 1.0

CelestinaSurface {
    id: root

    required property string deviceName
    required property string stateText
    required property string mountPath
    required property string fingerprint
    required property string batteryText

    readonly property bool mounted: mountPath.length > 0

    height: 96
    role: CelestinaSurface.Tonal

    Column {
        anchors.left: parent.left
        anchors.leftMargin: 18
        anchors.right: parent.right
        anchors.rightMargin: 18
        anchors.verticalCenter: parent.verticalCenter
        spacing: 3

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
            text: root.mounted ? root.stateText + " · " + root.mountPath : root.stateText
            color: root.mounted ? CelestinaTheme.accent : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideMiddle
        }

        Text {
            visible: root.batteryText.length > 0
            text: root.batteryText
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }

        Text {
            width: parent.width
            visible: root.fingerprint.length > 0
            text: "🔑 " + root.fingerprint
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.monoFamily
            font.pixelSize: CelestinaTheme.fontMini
            elide: Text.ElideRight
        }
    }
}
