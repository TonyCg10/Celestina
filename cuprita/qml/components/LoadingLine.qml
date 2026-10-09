import QtQuick
import org.celestina.cuprita 1.0

// The row a card shows while its first reading is under way: the spinner
// and a muted line, centred.
Item {
    id: line

    required property string text

    width: parent ? parent.width : 0
    height: CelestinaTheme.rowHeight
    Accessible.role: Accessible.StaticText
    Accessible.name: line.text

    Row {
        anchors.centerIn: parent
        spacing: CelestinaTheme.spaceSm

        Spinner {
            objectName: "loadingSpinner"
            anchors.verticalCenter: parent.verticalCenter
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: line.text
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
        }
    }
}
