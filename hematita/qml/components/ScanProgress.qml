import QtQuick
import org.celestina.hematita 1.0

// A running scan, in place of a panel's content: how much it has counted,
// where it is, and a way to stop it. The page composes the words; this only
// lays them out. The path is the filesystem's own, elided from the left so
// the folder being read stays visible.
Item {
    id: progress

    required property string counts
    required property string path

    signal cancelRequested()

    Column {
        anchors.centerIn: parent
        width: parent.width - CelestinaTheme.spaceLg * 2
        spacing: CelestinaTheme.spaceSm

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: CelestinaTheme.spaceSm

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: progress.counts
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.features: CelestinaTheme.fontFeaturesTabular
            }

            CelestinaIconButton {
                anchors.verticalCenter: parent.verticalCenter
                iconName: "x"
                helpText: qsTr("Cancelar")
                role: CelestinaButton.Ghost
                onClicked: progress.cancelRequested()
            }
        }

        Text {
            width: parent.width
            text: progress.path
            textFormat: Text.PlainText
            horizontalAlignment: Text.AlignHCenter
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideLeft
        }
    }
}
