import QtQuick
import org.celestina.magnetita 1.0

Item {
    id: root

    required property string deviceName
    required property string fingerprint
    required property bool online
    signal forgetRequested

    height: 66

    Rectangle {
        id: dot
        width: CelestinaTheme.compStatusIndicatorSize
        height: CelestinaTheme.compStatusIndicatorSize
        radius: height / 2
        color: root.online ? CelestinaTheme.success : CelestinaTheme.textMuted
        anchors.left: parent.left
        anchors.leftMargin: 16
        anchors.verticalCenter: parent.verticalCenter
    }

    Column {
        anchors.left: dot.right
        anchors.leftMargin: 12
        anchors.right: forget.left
        anchors.rightMargin: 12
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
            text: root.fingerprint.length > 0
                  ? "Huella del certificado · " + root.fingerprint
                  : "Huella del certificado no disponible"
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.monoFamily
            font.pixelSize: CelestinaTheme.fontMini
            font.features: CelestinaTheme.fontFeaturesTabular
            elide: Text.ElideRight
        }
    }

    CelestinaButton {
        id: forget
        anchors.right: parent.right
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        width: 100
        text: "Olvidar"
        onClicked: root.forgetRequested()
    }
}
