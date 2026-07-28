import QtQuick
import org.celestina.magnetita 1.0

CelestinaSurface {
    id: root

    required property var devices

    role: CelestinaSurface.Content

    ListView {
        id: logList
        anchors.fill: parent
        anchors.margins: 12
        spacing: 5
        clip: true
        model: root.devices.logLines

        delegate: Text {
            required property int index
            required property string modelData
            readonly property bool failure: index < root.devices.logFailures.length
                                               && root.devices.logFailures[index] === "true"
            width: logList.width
            text: modelData
            color: failure ? CelestinaTheme.danger : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            wrapMode: Text.WordWrap
        }
    }

    Text {
        anchors.centerIn: parent
        visible: root.devices.logLines.length === 0
        text: "Sin actividad todavía"
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
    }
}
