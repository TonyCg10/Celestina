import QtQuick
import org.celestina.magnetita 1.0

CelestinaSurface {
    id: root

    required property DevicesModel devices

    role: CelestinaSurface.Grouped

    Item {
        id: sectionHeader
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.leftMargin: 15
        anchors.rightMargin: 15
        height: 42

        CelestinaSectionLabel {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            text: "ACTIVIDAD"
        }

        Text {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: "Hoy"
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontMini
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: CelestinaTheme.borderHairline
            color: CelestinaTheme.divider
        }
    }

    ListView {
        id: logList
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: sectionHeader.bottom
        anchors.bottom: parent.bottom
        anchors.leftMargin: 13
        anchors.rightMargin: 13
        clip: true
        visible: root.devices.logAvailable
        model: root.devices.logLines

        delegate: Item {
            id: logRow
            required property int index
            required property string modelData
            readonly property bool failure: index < root.devices.logFailures.length
                                                   && root.devices.logFailures[index] === "true"
            width: logList.width
            height: CelestinaTheme.rowHeight

            Rectangle {
                id: logIcon
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.controlHeightXs
                height: width
                radius: CelestinaTheme.radiusSm
                color: logRow.failure ? CelestinaTheme.dangerFill
                                      : CelestinaTheme.accentSoft

                CelestinaIcon {
                    anchors.centerIn: parent
                    width: CelestinaTheme.iconSm
                    height: width
                    name: logRow.failure ? "dialog-error" : "emblem-synchronizing"
                    fallbackName: logRow.failure ? "file" : "view-refresh"
                    tone: logRow.failure ? CelestinaIcon.Danger : CelestinaIcon.Accent
                }
            }

            Text {
                // Peer-supplied text: never interpreted as markup.
                textFormat: Text.PlainText
                anchors.left: logIcon.right
                anchors.leftMargin: 10
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: logRow.modelData
                color: logRow.failure ? CelestinaTheme.dangerFillInk
                                      : CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                elide: Text.ElideRight
            }

            Rectangle {
                anchors.left: logIcon.right
                anchors.leftMargin: 10
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: CelestinaTheme.borderHairline
                color: CelestinaTheme.divider
            }
        }
    }

    Text {
        anchors.centerIn: logList
        visible: !root.devices.logAvailable
                 || root.devices.logLines.length === 0
        text: root.devices.logAvailable
              ? "Sin actividad todavía" : "Actividad no disponible"
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
    }
}
