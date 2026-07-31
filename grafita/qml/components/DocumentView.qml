import QtQuick
import QtQuick.Dialogs
import org.celestina.grafita 1.0
import org.celestina.grafita.internal 1.0

// The window's whole content: a header, the editing surface, a footer — or,
// with no document open, a plain invitation and whatever went wrong last.
//
// The text widget does not own the text. It shows `grafita-core`'s line-feed
// projection and reports its whole content back on every change; the core turns
// that into the one splice it represents, which is why editing a CRLF file here
// does not rewrite its line endings.
Item {
    id: root

    required property var session
    // True while the unsaved-work question is up: the document surface stops
    // accepting actions so the question is the only thing that can be answered.
    required property bool blocked

    // Called by the window when the document's text moved underneath the widget.
    function adopt(text, caret) {
        body.text = text
        body.cursorPosition = Math.min(caret, body.length)
        if (!root.blocked && !findBar.shown)
            body.forceActiveFocus()
    }

    // A search hit or a go-to-line: select it and bring it into view without
    // stealing the keyboard from the find bar, which the user is still typing in.
    function select(start, end) {
        body.select(Math.min(start, body.length), Math.min(end, body.length))
        scroller.revealCursor(body.positionToRectangle(body.selectionEnd))
    }

    function openFind(withReplace) {
        findBar.replacing = withReplace === true || findBar.replacing
        findBar.shown = true
        findBar.takeFocus()
    }

    DocumentHeader {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        session: root.session
    }

    FindBar {
        id: findBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        session: root.session
        enabled: !root.blocked
        onDismissed: {
            findBar.shown = false
            body.forceActiveFocus()
        }
    }

    Rectangle {
        id: page
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: CelestinaTheme.spaceLg
        anchors.rightMargin: CelestinaTheme.spaceLg
        anchors.top: findBar.bottom
        anchors.bottom: footer.top
        anchors.bottomMargin: CelestinaTheme.spaceSm
        visible: root.session.active
        enabled: !root.blocked
        color: CelestinaTheme.inputFill
        radius: CelestinaTheme.radiusInput
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.inputBorder

        // No focus ring, deliberately: the ring is reserved for keyboard focus,
        // and a bare `TextEdit` is a TextInput template with no `focusReason` —
        // the signal CelestinaTextField uses to tell Tab from a click. A ring
        // here could only be one that also fires on every click. On an editing
        // surface the caret is the focus affordance.

        Flickable {
            id: scroller
            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceSm
            clip: true
            contentWidth: width
            contentHeight: body.paintedHeight

            // Keep the caret on screen without animating the viewport, which
            // would be motion the user did not ask for.
            function revealCursor(rectangle) {
                if (rectangle.y < contentY)
                    contentY = rectangle.y
                else if (rectangle.y + rectangle.height > contentY + height)
                    contentY = rectangle.y + rectangle.height - height
            }

            // The line the caret is on. Painted behind the text and only while
            // nothing is selected: over a selection it would fight the
            // selection colour for the same pixels.
            Rectangle {
                width: scroller.width
                height: body.cursorRectangle.height
                y: body.cursorRectangle.y
                visible: body.activeFocus && !body.selectedText
                color: CelestinaTheme.surfaceHover
                radius: CelestinaTheme.radiusXs
            }

            TextEdit {
                id: body
                width: scroller.width
                wrapMode: TextEdit.Wrap
                selectByMouse: true
                selectByKeyboard: true
                persistentSelection: true
                color: CelestinaTheme.text
                selectionColor: CelestinaTheme.accent
                selectedTextColor: CelestinaTheme.accentInk
                font.family: CelestinaTheme.monoFamily
                font.pixelSize: CelestinaTheme.fontCaption

                Accessible.role: Accessible.EditableText
                Accessible.name: root.session.active
                                 ? "Contenido de " + root.session.name : ""

                // The core is the document; this reports what the widget now
                // holds and lets the core work out the edit.
                onTextChanged: root.session.applyText(text)
                onCursorRectangleChanged: scroller.revealCursor(cursorRectangle)

                // Colour without touching the text. A QSyntaxHighlighter applies
                // formats to the document's blocks and leaves the characters
                // alone, so what this widget reports back is still exactly the
                // core's projection — anything that rewrote the text as markup
                // would break the reconciliation instead.
                SyntaxHighlighter {
                    target: body.textDocument
                    language: root.session.languageId
                    commentColor: CelestinaTheme.codeComment
                    stringColor: CelestinaTheme.codeString
                    numberColor: CelestinaTheme.codeNumber
                    keywordColor: CelestinaTheme.codeKeyword
                }
            }
        }
    }

    // The document chooser. On Wayland this is the XDG portal, so the dialog it
    // shows is whichever backend the session routes there — Siderita's own, as
    // it happens. Grafita asks over the standard and does not care.
    FileDialog {
        id: openDialog
        title: "Abrir documento"
        onAccepted: root.session.openUrl(openDialog.selectedFile.toString())
    }

    // Nothing open: say so, offer the way in, and say what went wrong if
    // something did. An editor with no document and no button is a dead end.
    Column {
        anchors.centerIn: parent
        width: Math.min(420, parent.width - CelestinaTheme.space3xl)
        spacing: CelestinaTheme.spaceSm
        visible: !root.session.active

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: "Grafita abre un documento de texto."
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody

            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        Item {
            width: parent.width
            height: openButton.height

            CelestinaButton {
                id: openButton
                anchors.horizontalCenter: parent.horizontalCenter
                text: "Abrir archivo…"
                role: CelestinaButton.Primary
                enabled: !root.session.busy
                onClicked: openDialog.open()
            }
        }

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            visible: root.session.errorText.length > 0
            text: root.session.errorText
            color: CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption

            Accessible.role: Accessible.AlertMessage
            Accessible.name: "Error: " + text
        }
    }

    DocumentFooter {
        id: footer
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        session: root.session
        enabled: !root.blocked
    }
}
