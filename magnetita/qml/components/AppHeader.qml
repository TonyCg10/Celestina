import QtQuick
import org.celestina.magnetita 1.0

Item {
    id: root

    required property bool settingsOpen
    required property int deviceCount
    required property bool devicesAvailable
    required property bool settingsAvailable
    signal toggleRequested

    height: 92

    Column {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        CelestinaSectionLabel {
            text: root.settingsOpen ? "PREFERENCIAS" : "CELESTINA LINK"
        }

        Text {
            text: root.settingsOpen ? "Ajustes" : "Magnetita"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontHeaderExpanded
            font.weight: CelestinaTheme.weightDemiBold
        }

        Text {
            text: root.settingsOpen
                  ? root.settingsAvailable
                    ? "Dispositivos y plugins"
                    : "Servicio no disponible"
                  : !root.devicesAvailable
                    ? "Servicio no disponible"
                    : root.deviceCount > 0
                    ? root.deviceCount + (root.deviceCount === 1
                        ? " dispositivo conectado"
                        : " dispositivos conectados")
                    : "Ningún dispositivo conectado"
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }
    }

    CelestinaIconButton {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        density: CelestinaButton.Regular
        iconName: root.settingsOpen ? "go-previous" : "preferences-system"
        fallbackIcon: root.settingsOpen ? "go-previous" : "settings"
        helpText: root.settingsOpen ? "Volver" : "Ajustes"
        onClicked: root.toggleRequested()
    }
}
