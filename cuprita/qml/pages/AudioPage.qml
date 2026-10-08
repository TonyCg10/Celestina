import QtQuick
import org.celestina.cuprita 1.0

// The Audio section. Empty until its CUP-1 unit gives it content.
CelestinaSurface {
    role: CelestinaSurface.Grouped

    Accessible.name: qsTr("Audio")

    Text {
        anchors.centerIn: parent
        text: qsTr("Audio")
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
    }
}
