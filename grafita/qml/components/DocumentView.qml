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
    // How the user reads: text size and wrapping. Owned and stored by the
    // window, because that is a property of the reader rather than of whichever
    // document happens to be in front.
    required property var reading

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

        // The numbers sit outside the Flickable, not inside it. With wrapping
        // off the content scrolls sideways, and a gutter that travelled with it
        // would take the numbers off the left edge of the window.
        CelestinaLineGutter {
            id: gutter
            anchors.left: parent.left
            // The text is set in from the border rather than run against it: a
            // caret or a descender touching the frame reads as a rendering
            // fault, and the first column needs somewhere to breathe.
            anchors.leftMargin: CelestinaTheme.spaceMd
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.topMargin: CelestinaTheme.spaceMd
            anchors.bottomMargin: CelestinaTheme.spaceMd
            surface: body
            viewportY: scroller.contentY
            viewportHeight: scroller.height
        }

        Flickable {
            id: scroller
            // A gap wide enough that a number and the line it belongs to do
            // not read as one string.
            anchors.left: gutter.right
            anchors.leftMargin: CelestinaTheme.spaceMd
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.rightMargin: CelestinaTheme.spaceMd
            anchors.topMargin: CelestinaTheme.spaceMd
            anchors.bottomMargin: CelestinaTheme.spaceMd
            clip: true
            contentWidth: body.width
            contentHeight: body.paintedHeight
            // Wrapped text has nothing to scroll to sideways, so the viewport
            // is pinned rather than left free to drift off the first column.
            boundsBehavior: Flickable.StopAtBounds

            // Ctrl and the wheel resize the text instead of scrolling it.
            // Deltas are accumulated to a full notch so a touchpad's stream of
            // small values moves one step at a time rather than sweeping
            // through the whole range in a gesture.
            WheelHandler {
                property real pending: 0
                readonly property real notch: 120

                acceptedModifiers: Qt.ControlModifier
                onWheel: function(event) {
                    pending += event.angleDelta.y
                    while (pending >= notch) {
                        pending -= notch
                        root.reading.enlargeText()
                    }
                    while (pending <= -notch) {
                        pending += notch
                        root.reading.shrinkText()
                    }
                }
            }

            // Keep the caret on screen without animating the viewport, which
            // would be motion the user did not ask for.
            function revealCursor(rectangle) {
                if (rectangle.y < contentY)
                    contentY = rectangle.y
                else if (rectangle.y + rectangle.height > contentY + height)
                    contentY = rectangle.y + rectangle.height - height
                if (rectangle.x < contentX)
                    contentX = rectangle.x
                else if (rectangle.x + rectangle.width > contentX + width)
                    contentX = rectangle.x + rectangle.width - width
            }

            // The line the caret is on. Painted behind the text and only while
            // nothing is selected: over a selection it would fight the
            // selection colour for the same pixels.
            Rectangle {
                width: Math.max(scroller.width, body.width)
                height: body.cursorRectangle.height
                y: body.cursorRectangle.y
                visible: body.activeFocus && !body.selectedText
                color: CelestinaTheme.surfaceHover
                radius: CelestinaTheme.radiusXs
            }

            TextEdit {
                id: body
                // Wrapped, the surface is exactly the viewport. Unwrapped, it
                // is as wide as its longest line, which is what gives the
                // Flickable something to scroll sideways through.
                width: root.reading.wrap
                       ? scroller.width
                       : Math.max(scroller.width, body.implicitWidth)
                wrapMode: root.reading.wrap ? TextEdit.Wrap : TextEdit.NoWrap
                selectByMouse: true
                selectByKeyboard: true
                persistentSelection: true
                color: CelestinaTheme.text
                selectionColor: CelestinaTheme.accent
                selectedTextColor: CelestinaTheme.accentInk
                font.family: CelestinaTheme.monoFamily
                font.pixelSize: root.reading.fontSize

                Accessible.role: Accessible.EditableText
                Accessible.name: root.session.active
                                 ? "Contenido de " + root.session.name : ""

                // The core is the document; this reports what the widget now
                // holds and lets the core work out the edit.
                onTextChanged: root.session.applyText(text)
                onCursorRectangleChanged: scroller.revealCursor(cursorRectangle)
                // The widget knows where its caret is as a UTF-16 offset; only
                // the document can say which line and column that is.
                onCursorPositionChanged: root.session.setCaret(cursorPosition)

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

        // Over the viewport's edges rather than beside them: the text keeps its
        // full width, and a bar exists only while there is something to scroll.
        // The vertical one stops short of the horizontal one so the two never
        // overlap in the corner.
        CelestinaScrollBar {
            surface: scroller
            anchors.right: scroller.right
            anchors.top: scroller.top
            anchors.bottom: scroller.bottom
            anchors.bottomMargin: sideways.visible ? sideways.height : 0
        }

        CelestinaScrollBar {
            id: sideways
            horizontal: true
            surface: scroller
            anchors.left: scroller.left
            anchors.right: scroller.right
            anchors.bottom: scroller.bottom
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

    // The destination chooser was dismissed, so whatever was waiting on the
    // write — a tab closing, a quit sweep — has to stop waiting for it.
    signal saveCancelled()

    // Where a document with no file yet goes. Reached by saving one, so the
    // question is asked at the moment the user actually wants an answer.
    //
    // The chosen URL is handed over whole. Percent-decoding it and deciding
    // what counts as a local file are document rules with one owner in the
    // core, not something a surface may take apart with `substring`.
    FileDialog {
        id: saveDialog
        title: "Guardar como"
        fileMode: FileDialog.SaveFile
        onAccepted: root.session.saveUrl(saveDialog.selectedFile.toString())
        onRejected: {
            root.session.cancelSaveAs()
            root.saveCancelled()
        }
    }

    /// Called by the window when the document says it has nowhere to go.
    function askDestination() {
        saveDialog.open()
    }

    // The documents this window could pick up again. Re-read whenever the empty
    // state appears rather than held: another window may have opened something
    // since, and a stale history is the one thing a history must not be.
    property var recentPaths: []
    function refreshRecent() {
        root.recentPaths = root.session.recentDocuments()
    }
    onVisibleChanged: if (visible) root.refreshRecent()
    Connections {
        target: root.session
        function onActiveChanged() { if (!root.session.active) root.refreshRecent() }
    }
    Component.onCompleted: root.refreshRecent()

    function baseName(path) {
        const cut = path.lastIndexOf("/")
        return cut >= 0 ? path.substring(cut + 1) : path
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

            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: CelestinaTheme.spaceSm

                CelestinaButton {
                    id: openButton
                    text: "Abrir archivo…"
                    role: CelestinaButton.Primary
                    enabled: !root.session.busy
                    onClicked: openDialog.open()
                }

                CelestinaButton {
                    text: "Documento nuevo"
                    enabled: !root.session.busy
                    onClicked: root.session.newDocument()
                }
            }
        }

        // Recent documents, if there are any. A new tab that remembers what you
        // were working on beats a new tab that makes you go find it again.
        Column {
            width: parent.width
            spacing: CelestinaTheme.spaceXs
            visible: root.recentPaths.length > 0

            Item { width: 1; height: CelestinaTheme.spaceMd }

            CelestinaSectionLabel {
                text: "Recientes"
            }

            Repeater {
                model: root.recentPaths

                delegate: Rectangle {
                    id: entry
                    required property string modelData

                    width: parent.width
                    height: CelestinaTheme.controlHeight
                    radius: CelestinaTheme.radiusSm
                    color: hover.hovered ? CelestinaTheme.surfaceHover
                                         : CelestinaTheme.withAlpha(CelestinaTheme.surface, 0)

                    Accessible.role: Accessible.Button
                    Accessible.name: "Abrir " + root.baseName(entry.modelData)
                    Accessible.description: entry.modelData

                    HoverHandler { id: hover }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: root.session.openPath(entry.modelData)
                    }

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: CelestinaTheme.spaceSm
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - CelestinaTheme.space2xl
                        elide: Text.ElideMiddle
                        // The name reads first; the folder is there to tell two
                        // files of the same name apart.
                        text: root.baseName(entry.modelData)
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                    }
                }
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
