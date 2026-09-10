import QtQuick
import QtQuick.Layouts
import org.celestina.magnetita 1.0

// One registered command: its name for the phone, the line it runs, and
// the one action a row offers, removing it.
Item {
    id: root

    required property string name
    required property string line
    signal removeRequested

    height: 56

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: CelestinaTheme.spaceLg
        anchors.rightMargin: CelestinaTheme.spaceSm
        spacing: CelestinaTheme.spaceSm

        Column {
            Layout.fillWidth: true
            spacing: 2

            Text {
                textFormat: Text.PlainText
                width: parent.width
                text: root.name
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.weight: CelestinaTheme.weightDemiBold
                elide: Text.ElideRight
            }

            Text {
                textFormat: Text.PlainText
                width: parent.width
                text: root.line
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.monoFamily
                font.pixelSize: CelestinaTheme.fontMini
                elide: Text.ElideRight
            }
        }

        CelestinaIconButton {
            iconName: "list-x"
            helpText: qsTr("Quitar la orden")
            onClicked: root.removeRequested()
        }
    }
}
