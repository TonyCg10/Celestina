import QtQuick
import org.celestina.cuprita 1.0

// The Bluetooth section. Empty until its CUP-1 unit gives it content.
CelestinaSurface {
    role: CelestinaSurface.Grouped

    Accessible.name: qsTr("Bluetooth")

    Text {
        anchors.centerIn: parent
        text: qsTr("Bluetooth")
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
    }
}
