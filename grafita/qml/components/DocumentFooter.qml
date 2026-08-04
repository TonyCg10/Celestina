import QtQuick
import org.celestina.grafita 1.0

// Where the caret is and what just happened on the left, what can be done on
// the right. A refusal outranks a completed action: if the last write did not
// happen, that is the sentence the user needs.
Item {
    id: root

    required property var session

    implicitHeight: actions.height + CelestinaTheme.space2xl

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

    Row {
        id: actions
        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceLg
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceMd
        spacing: CelestinaTheme.spaceSm
        visible: root.session.active

        CelestinaButton {
            text: "Deshacer"
            enabled: root.session.canUndo
            onClicked: root.session.undo()
        }
        CelestinaButton {
            text: "Rehacer"
            enabled: root.session.canRedo
            onClicked: root.session.redo()
        }
        CelestinaButton {
            text: "Cerrar"
            onClicked: root.session.requestClose()
        }
        CelestinaButton {
            text: "Guardar"
            role: CelestinaButton.Primary
            enabled: root.session.dirty && !root.session.busy
            onClicked: root.session.save()
        }
    }
}
