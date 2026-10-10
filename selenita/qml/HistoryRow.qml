import QtQuick
import org.celestina.selenita 1.0
import "components"

// One capture in the history: its thumbnail, its name, what it is, its size
// and when it was taken, and the four actions on it. The row is a Tab stop
// that reads as one list item; on it, Enter opens the file, Delete asks for
// the trash, and the arrows, Home and End ask the card for another row.
Item {
    id: row

    // `{ id, name, url, kind, size, takenAt }` from the controller.
    required property var entry
    // The card's `rowInset`.
    required property int inset
    property bool separated: false

    // Keyboard focus, as the style's controls show it: a ring for a Tab or a
    // key, none for a click. An Item has no `focusReason`, so the row notes
    // the click itself and forgets it when the focus leaves.
    property bool pointerFocused: false
    readonly property bool visualFocus: row.activeFocus && !row.pointerFocused
    readonly property string kindText: row.entry.kind === "recording" ? qsTr("Grabación")
                                                                       : qsTr("Captura")
    readonly property string takenText: Qt.formatDateTime(new Date(row.entry.takenAt),
                                                          "d MMM, HH:mm")

    // The focus should move `delta` rows (-1 or 1).
    signal walkRequested(int delta)
    // The focus should move to the first row, or the last.
    signal edgeRequested(bool last)
    // The entry should go to the trash.
    signal trashRequested()

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
    activeFocusOnTab: true
    Accessible.role: Accessible.ListItem
    Accessible.focusable: true
    Accessible.name: [row.entry.name, row.kindText, row.sizeText(row.entry.size),
                      row.takenText].join(", ")
    Accessible.description: qsTr("Intro abre en Fluorita; Suprimir mueve a la papelera")
    Accessible.onPressAction: SelenitaController.openInFluorita(row.entry.id)

    onActiveFocusChanged: {
        if (!row.activeFocus)
            row.pointerFocused = false
    }

    // Enter opens only when the row itself holds the focus: from an action
    // button inside it the key goes on to the window's map, which presses
    // the button.
    function open(event: KeyEvent) {
        if (row.Window.activeFocusItem === row)
            SelenitaController.openInFluorita(row.entry.id)
        else
            event.accepted = false
    }

    Keys.onReturnPressed: event => row.open(event)
    Keys.onEnterPressed: event => row.open(event)
    Keys.onDeletePressed: row.trashRequested()
    Keys.onUpPressed: row.walkRequested(-1)
    Keys.onDownPressed: row.walkRequested(1)
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Home)
            row.edgeRequested(false)
        else if (event.key === Qt.Key_End)
            row.edgeRequested(true)
        else
            return
        event.accepted = true
    }

    RowDivider {
        inset: row.inset
        visible: row.separated
    }

    HoverHandler { id: hover }

    // A click on the row's open ground selects it for the keys above; the
    // actions take their own clicks first.
    TapHandler {
        onTapped: {
            row.pointerFocused = true
            row.forceActiveFocus(Qt.MouseFocusReason)
        }
    }

    CelestinaRowHighlight {
        anchors.fill: parent
        radius: CelestinaTheme.radiusMd
        family: CelestinaRowHighlight.Content
        hovered: hover.hovered
        focused: row.visualFocus
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

        // The row reads both lines as one name.
        Text {
            width: parent.width
            text: row.entry.name
            elide: Text.ElideMiddle
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            Accessible.ignored: true
        }

        Text {
            objectName: "details"
            width: parent.width
            text: [row.kindText, row.sizeText(row.entry.size), row.takenText].join(" · ")
            elide: Text.ElideRight
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            Accessible.ignored: true
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
