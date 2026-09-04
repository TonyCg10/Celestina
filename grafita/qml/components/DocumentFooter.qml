import QtQuick
import org.celestina.grafita 1.0

// Where the caret is and what just happened on the left, what can be done on
// the right. A refusal outranks a completed action: if the last write did not
// happen, that is the sentence the user needs.
Item {
    id: root

    required property var session

    implicitHeight: actions.height + CelestinaTheme.space2xl

    // Whether the document may be re-read as another encoding right now. One
    // owner for the rule: the button below and the window's Ctrl+E both read
    // it, so the shortcut can never do what the button refuses.
    //
    // A document with unsaved work cannot be re-read without losing them, so
    // the action is not offered rather than offered and refused. An imported
    // document has no encoding to choose either: it says what container it
    // came out of and stops there.
    readonly property bool encodingChoosable: !root.session.dirty && !root.session.busy
                                              && !root.session.imported

    // The caret's position, which is what a person quotes when they talk about
    // a place in a file. Both numbers come from the document rather than from
    // the widget: the column counts characters, so an accented letter is one
    // column and not the two bytes it occupies.
    Text {
        id: caret
        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceLg
        anchors.verticalCenter: actions.verticalCenter
        visible: root.session.active
        text: "Ln " + root.session.caretLine + ", Col " + root.session.caretColumn
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.monoFamily
        font.pixelSize: CelestinaTheme.fontCaption

        // Spelled out for assistive technology, where "Ln" and "Col" would be
        // read as words. English because the repository language standard
        // admits no other; the visible abbreviations above are neutral.
        Accessible.role: Accessible.StaticText
        Accessible.name: "Line " + root.session.caretLine
                         + ", column " + root.session.caretColumn
    }

    Text {
        id: status
        anchors.left: caret.visible ? caret.right : parent.left
        anchors.leftMargin: CelestinaTheme.spaceLg
        anchors.right: actions.left
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: actions.verticalCenter
        elide: Text.ElideRight
        text: {
            if (root.session.errorText.length > 0 && root.session.active)
                return root.session.errorText
            if (root.session.busy)
                return "Trabajando…"
            return root.session.statusText
        }
        color: root.session.errorText.length > 0 && root.session.active
               ? CelestinaTheme.danger : CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption

        Accessible.role: root.session.errorText.length > 0 && root.session.active
                         ? Accessible.AlertMessage : Accessible.StaticText
        Accessible.name: status.text
        Accessible.ignored: status.text.length === 0
    }

    // Back in the footer because it now asks for something. G7 removed this
    // label when it was only a statement about the document; naming the
    // encoding is an action, and this is where the document's actions live.
    CelestinaButton {
        id: encodingButton
        anchors.right: actions.left
        anchors.rightMargin: CelestinaTheme.spaceSm
        anchors.verticalCenter: actions.verticalCenter
        visible: root.session.active || root.session.encodingRetry.length > 0
        enabled: root.encodingChoosable
        text: {
            if (root.session.imported)
                return root.session.containerLabel
            return root.session.active
                   ? root.session.encodingLabel : "Leer como…"
        }
        onClicked: root.session.requestEncodingChooser()

        Accessible.role: Accessible.Button
        Accessible.name: "Read this document as another encoding"

        // The encoding name is the information, so the label stays; the glyph
        // in front says what kind of thing it names.
        contentItem: Row {
            spacing: CelestinaTheme.spaceXs

            CelestinaIcon {
                anchors.verticalCenter: parent.verticalCenter
                name: "binary"
                tone: encodingButton.enabled ? CelestinaIcon.Primary
                                             : CelestinaIcon.Secondary
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: encodingButton.text
                textFormat: Text.PlainText
                font: encodingButton.font
                color: encodingButton.enabled ? CelestinaTheme.text
                                              : CelestinaTheme.textMuted
            }
        }
    }

    Row {
        id: actions
        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceLg
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceMd
        spacing: CelestinaTheme.spaceSm
        visible: root.session.active

        // Undo and redo are one pair, so they share one capsule; the shortcuts
        // named in the tooltips are the ones Main.qml binds.
        CelestinaCapsule {
            anchors.verticalCenter: parent.verticalCenter

            CelestinaIconButton {
                iconName: "undo"
                role: CelestinaButton.Ghost
                helpText: "Deshacer (Ctrl+Z)"
                enabled: root.session.canUndo
                onClicked: root.session.undo()
            }
            CelestinaIconButton {
                iconName: "redo"
                role: CelestinaButton.Ghost
                helpText: "Rehacer (Ctrl+Shift+Z)"
                enabled: root.session.canRedo
                onClicked: root.session.redo()
            }
        }
        CelestinaIconButton {
            anchors.verticalCenter: parent.verticalCenter
            iconName: "x"
            helpText: "Cerrar (Ctrl+W)"
            onClicked: root.session.requestClose()
        }
        CelestinaIconButton {
            anchors.verticalCenter: parent.verticalCenter
            iconName: "save"
            role: CelestinaButton.Primary
            helpText: "Guardar (Ctrl+S)"
            enabled: root.session.dirty && !root.session.busy
            onClicked: root.session.save()
        }
    }
}
