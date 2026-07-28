import QtQuick
import org.celestina.magnetita 1.0

GlassSurface {
    id: root

    required property bool settingsOpen
    signal toggleRequested

    height: 46
    cornerRadius: CelestinaTheme.radiusPill
    elevation: 2
    liveCapture: false

    Text {
        anchors.left: parent.left
        anchors.leftMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        text: root.settingsOpen ? "Ajustes" : "Magnetita"
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontHeaderCollapsed
        font.weight: CelestinaTheme.weightDemiBold
    }

    CelestinaButton {
        anchors.right: parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        width: 116
        text: root.settingsOpen ? "‹ Volver" : "⚙ Ajustes"
        onClicked: root.toggleRequested()
    }
}
