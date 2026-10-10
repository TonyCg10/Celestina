import QtQuick
import org.celestina.selenita 1.0
import "components"

// One capture in the history: its thumbnail, its name, what it is, its size
// and when it was taken, and the four actions on it.
Item {
    id: row

    // `{ id, name, url, kind, size, takenAt }` from the controller.
    required property var entry
    // The card's `rowInset`.
    required property int inset
    property bool separated: false

    // A size in the units a person reads.
    function sizeText(bytes: real): string {
        if (bytes < 1024)
            return qsTr("%1 B").arg(bytes)
        if (bytes < 1024 * 1024)
            return qsTr("%1 KB").arg((bytes / 1024).toFixed(0))
        return qsTr("%1 MB").arg((bytes / (1024 * 1024)).toFixed(1))
    }

    width: parent ? parent.width : 0
    height: CelestinaTheme.rowHeightLg + CelestinaTheme.spaceSm
    Accessible.role: Accessible.ListItem
    Accessible.name: row.entry.name

    RowDivider {
        inset: row.inset
        visible: row.separated
    }

    HoverHandler { id: hover }

    CelestinaRowHighlight {
        anchors.fill: parent
        radius: CelestinaTheme.radiusMd
        family: CelestinaRowHighlight.Content
        hovered: hover.hovered
    }

    Rectangle {
        id: frame
        anchors.left: parent.left
        anchors.leftMargin: row.inset
        anchors.verticalCenter: parent.verticalCenter
        width: CelestinaTheme.glyphTileLg
        height: CelestinaTheme.glyphTile
        radius: CelestinaTheme.radiusSm
        color: CelestinaTheme.controlFill
        clip: true

        // A recording has no still to show: the film glyph stands for it.
        readonly property bool isRecording: row.entry.kind === "recording"

        Image {
            objectName: "thumbnail"
            anchors.fill: parent
            visible: !frame.isRecording
            source: frame.isRecording ? "" : row.entry.url
            sourceSize.width: CelestinaTheme.glyphTileLg * 2
            sourceSize.height: CelestinaTheme.glyphTile * 2
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: false
            Accessible.ignored: true
        }

        CelestinaIcon {
            objectName: "filmGlyph"
            anchors.centerIn: parent
            visible: frame.isRecording
            name: "film"
            width: CelestinaTheme.iconMd
            height: width
            Accessible.ignored: true
        }
    }

    Column {
        anchors.left: frame.right
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.right: actions.left
        anchors.rightMargin: CelestinaTheme.spaceSm
        anchors.verticalCenter: parent.verticalCenter

        Text {
            width: parent.width
            text: row.entry.name
            elide: Text.ElideMiddle
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
        }

        Text {
            objectName: "details"
            width: parent.width
            text: [row.entry.kind === "recording" ? qsTr("Grabación") : qsTr("Captura"),
                   row.sizeText(row.entry.size),
                   Qt.formatDateTime(new Date(row.entry.takenAt), "d MMM, HH:mm")].join(" · ")
            elide: Text.ElideRight
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }
    }

    Row {
        id: actions
        anchors.right: parent.right
        anchors.rightMargin: row.inset
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceXs

        CelestinaIconButton {
            objectName: "openAction"
            iconName: "image"
            role: CelestinaButton.Ghost
            helpText: qsTr("Abrir en Fluorita")
            onClicked: SelenitaController.openInFluorita(row.entry.id)
        }

        // Only a picture goes to the clipboard.
        CelestinaIconButton {
            objectName: "copyAction"
            visible: row.entry.kind !== "recording"
            iconName: "copy"
            role: CelestinaButton.Ghost
            helpText: qsTr("Copiar")
            onClicked: SelenitaController.copy(row.entry.id)
        }

        CelestinaIconButton {
            objectName: "showAction"
            iconName: "folder-open"
            role: CelestinaButton.Ghost
            helpText: qsTr("Mostrar en Siderita")
            onClicked: SelenitaController.showInSiderita(row.entry.id)
        }

        CelestinaIconButton {
            objectName: "deleteAction"
            iconName: "user-trash"
            role: CelestinaButton.Ghost
            helpText: qsTr("Mover a la papelera")
            onClicked: SelenitaController.deleteEntry(row.entry.id)
        }
    }
}
