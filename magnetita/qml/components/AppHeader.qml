import QtQuick
import org.celestina.magnetita 1.0

Item {
    id: root

    required property bool settingsOpen
    required property bool messagesOpen
    required property int deviceCount
    required property bool devicesAvailable
    required property bool settingsAvailable
    signal toggleRequested
    signal messagesRequested

    height: 92

    Column {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        CelestinaSectionLabel {
            text: root.settingsOpen ? "PREFERENCIAS" : root.messagesOpen ? "MENSAJES" : "CELESTINA LINK"
        }

        Text {
            text: root.settingsOpen ? "Ajustes" : root.messagesOpen ? "SMS" : "Magnetita"
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
                  : root.messagesOpen
                  ? "Las conversaciones del m\u00f3vil"
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

    Row {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceXs

        // Messages and settings are the two other pages; whichever is open
        // shows the way back in its own place.
        CelestinaIconButton {
            density: CelestinaButton.Regular
            visible: !root.settingsOpen
            iconName: root.messagesOpen ? "go-previous" : "mail"
            fallbackIcon: root.messagesOpen ? "go-previous" : "mail"
            helpText: root.messagesOpen ? "Volver" : "Mensajes"
            onClicked: root.messagesRequested()
        }

        CelestinaIconButton {
            density: CelestinaButton.Regular
            visible: !root.messagesOpen
            iconName: root.settingsOpen ? "go-previous" : "preferences-system"
            fallbackIcon: root.settingsOpen ? "go-previous" : "settings"
            helpText: root.settingsOpen ? "Volver" : "Ajustes"
            onClicked: root.toggleRequested()
        }
    }
}
