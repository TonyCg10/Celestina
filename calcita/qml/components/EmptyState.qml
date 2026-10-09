import QtQuick
import org.celestina.calcita 1.0

// What the window shows while no document is open: one grouped card with the
// state in words and the way out of it. `openRequested` asks for a document;
// the window answers it once the file chooser arrives (CAL-1-A).
Column {
    id: empty

    signal openRequested()

    // The words and the button sit this far inside the card's edge.
    readonly property int rowInset: CelestinaTheme.spaceLg

    objectName: "emptyState"
    spacing: CelestinaTheme.spaceSm

    CelestinaSurface {
        width: empty.width
        height: content.implicitHeight + empty.rowInset * 2
        role: CelestinaSurface.Grouped
        Accessible.ignored: true

        Column {
            id: content
            x: empty.rowInset
            y: empty.rowInset
            width: parent.width - empty.rowInset * 2
            spacing: CelestinaTheme.spaceMd

            Text {
                objectName: "emptyTitle"
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: qsTr("Sin documento")
                color: CelestinaTheme.text
                font.pixelSize: CelestinaTheme.fontTitle
                font.weight: CelestinaTheme.weightDemiBold
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                text: qsTr("Abre un PDF para leerlo aquí.")
                color: CelestinaTheme.textMuted
                font.pixelSize: CelestinaTheme.fontBody
            }

            CelestinaButton {
                objectName: "openButton"
                anchors.horizontalCenter: parent.horizontalCenter
                role: CelestinaButton.Primary
                text: qsTr("Abrir…")
                onClicked: empty.openRequested()
            }
        }
    }
}
