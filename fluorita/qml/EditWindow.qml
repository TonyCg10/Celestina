pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.fluorita 1.0

// One file, edited in a window of its own (ADR 0012, PRV-1).
//
// `Fluorita1.Edit` and `fluorita --edit` open one of these per file, beside
// the library window, which is left exactly as it was. The window has its own
// editor, so two pictures being edited never share a document, and it is the
// same editor the library uses: the same surface, the same two save outcomes,
// the same rules about what reaches disk and when.
//
// It is a top-level window with no parent window, titled after the file. That
// title is what the compositor matches: niri's rule for `Editar — ` opens it
// floating and centred; nothing here places it.
Window {
    id: editWindow

    // The file being edited, as its path key: byte-exact, never shown.
    required property string key
    // Whether the file is a video. Pictures only for now; the trim fills it.
    property bool video: false

    // This window's own editor, for the host and for tests to read.
    readonly property FluoritaEditor editor: editorObject
    // What a person reads in the title: the file's name, lossy on purpose.
    readonly property string fileName: editorObject.nameOf(editWindow.key)

    // The window is gone and its row can go: the host drops it later, never
    // from inside this window's own handler.
    signal finished()

    width: 960
    height: 680
    minimumWidth: 480
    minimumHeight: 360
    visible: true
    // A window of its own, never a dialog of the library's: with a parent it
    // would follow the library window around, and close with it.
    transientParent: null
    // Transparent: the compositor blurs what lies behind and the backdrop
    // paints the Haze canvas over it, as in the library window.
    color: CelestinaTheme.clear
    title: qsTr("Editar — %1").arg(editWindow.fileName)

    FluoritaEditor {
        id: editorObject
    }

    CelestinaBackdrop {
        anchors.fill: parent
    }

    EditSurface {
        id: surface

        objectName: "editSurface"
        anchors.fill: parent
        visible: editorObject.open
        focus: editorObject.open
        editor: editorObject
        onClosed: {
            editorObject.close()
            editWindow.close()
        }
    }

    // When the editor is not open: the saved result, or why nothing opened.
    Item {
        id: aftermath

        anchors.fill: parent
        anchors.margins: CelestinaTheme.space2xl
        visible: !editorObject.open

        readonly property bool saved: editorObject.savedKey !== ""

        // The result that landed, offered to other programs as a file: a
        // `text/uri-list` drag that only ever copies, so a target that asks
        // to move it is refused and the file stays where it was written. The
        // URL is the Rust codec's, not `encodeURI`'s, which would cut a name
        // at its `#`.
        Item {
            id: savedResult

            objectName: "savedResult"
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: notice.top
            anchors.bottomMargin: CelestinaTheme.spaceLg
            visible: aftermath.saved

            Drag.dragType: Drag.Automatic
            Drag.supportedActions: Qt.CopyAction
            Drag.proposedAction: Qt.CopyAction
            // The line ends in CR LF, as RFC 2483 asks. Spelled by code point:
            // qmlcachegen (Qt 6.12) copies a "\r\n" escape in this binding
            // into its generated C++ as a raw line break, and the build fails.
            Drag.mimeData: ({
                "text/uri-list": editorObject.savedUrl + String.fromCharCode(13, 10)
            })
            Drag.active: dragOut.active

            Accessible.role: Accessible.Graphic
            Accessible.name: qsTr("Resultado guardado: %1").arg(
                editorObject.nameOf(editorObject.savedKey))

            Image {
                anchors.fill: parent
                source: aftermath.saved ? Qt.resolvedUrl(editorObject.savedUrl) : ""
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                autoTransform: true
                cache: false
                // Decoded at the size it is shown, never at the file's own.
                sourceSize.width: Math.ceil(width * Screen.devicePixelRatio)
                sourceSize.height: Math.ceil(height * Screen.devicePixelRatio)
            }

            DragHandler {
                id: dragOut

                target: null
            }

            HoverHandler {
                cursorShape: dragOut.active ? Qt.ClosedHandCursor : Qt.OpenHandCursor
            }
        }

        // What the last save or open said.
        CelestinaSectionLabel {
            id: notice

            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: actions.top
            anchors.bottomMargin: CelestinaTheme.spaceMd
            visible: editorObject.notice !== ""
            text: editorObject.notice
        }

        Row {
            id: actions

            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            spacing: CelestinaTheme.spaceXs
            visible: aftermath.saved || editorObject.notice !== ""

            // The result reopens as what it is: a copy with its marks still
            // movable, or the flattened replacement.
            CelestinaIconButton {
                visible: aftermath.saved
                iconName: "pencil"
                helpText: qsTr("Seguir editando")
                onClicked: editorObject.openItem(editorObject.savedKey)
            }

            CelestinaIconButton {
                iconName: "x"
                helpText: qsTr("Cerrar")
                onClicked: editWindow.close()
            }
        }
    }

    Shortcut {
        sequences: [StandardKey.Close]
        onActivated: editWindow.close()
    }

    // Closing with unsaved changes asks first, and a save in flight is waited
    // for: the window never goes while the editor still holds something that
    // is not on disk.
    onClosing: function(close) {
        if (editorObject.saving) {
            close.accepted = false
            return
        }
        if (editorObject.open && editorObject.edited) {
            close.accepted = false
            surface.leave()
            return
        }
        editorObject.close()
        editWindow.finished()
    }

    Component.onCompleted: editorObject.openItem(editWindow.key)
}
