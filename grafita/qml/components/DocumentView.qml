// The recent-file delegates reach the view's `root` id, which a delegate may
// only do under bound component behaviour; each declares its own model row.
pragma ComponentBehavior: Bound

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
        // A box on the glass canvas is opaque, in the bar's black: the text
        // must never sit over whatever the compositor shows behind the window.
        color: CelestinaTheme.card
        radius: CelestinaTheme.radiusInput
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.inputBorder

        // No focus ring, deliberately: the ring is reserved for keyboard focus,
        // and a bare `TextEdit` is a TextInput template with no `focusReason` —
        // the signal CelestinaTextField uses to tell Tab from a click. A ring
        // here could only be one that also fires on every click. On an editing
        // surface the caret is the focus affordance.

        // The whole box is the editor, not only the painted lines. A click in
        // the margin or on the gutter lands here, under everything else, and
        // focuses the surface: on the gutter the caret goes to the start of
        // the line beside the pointer, anywhere else to the end of the text.
        // Clicks on the text itself never reach this — the widget takes them.
        //
        // Stacking order this relies on: declared first, so it sits under the
        // gutter, the Flickable and the TextEdit; a press lands on the topmost
        // item that accepts it, so only the frame outside the Flickable reaches
        // here. Composed events are not propagated: whatever this area takes,
        // it keeps — the default, made explicit.
        MouseArea {
            anchors.fill: parent
            propagateComposedEvents: false
            onClicked: function(mouse) {
                const inGutter = mouse.x < gutter.x + gutter.width
                const point = body.mapFromItem(page, mouse.x, mouse.y)
                body.cursorPosition = inGutter ? body.positionAt(0, point.y)
                                               : body.length
                body.forceActiveFocus()
            }
        }

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
            // The session already indexed every line while absorbing the last
            // edit; handing it over stops the gutter re-reading and
            // re-scanning the whole document per keystroke (GRA-P1).
            lineSource: root.session
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

            // Plain wheel scrolls the document; Ctrl+wheel is left to the
            // zoom handler above.
            CelestinaWheelScroll { view: scroller }

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

            // The matching pair of brackets around the caret, behind the text
            // like the caret's line, and shown under the same rule: only while
            // the text has the keyboard and nothing is selected.
            Repeater {
                model: body.bracketBoxes
                delegate: Rectangle {
                    required property rect modelData
                    x: body.x + modelData.x
                    y: body.y + modelData.y
                    width: modelData.width
                    height: modelData.height
                    visible: body.activeFocus && !body.selectedText
                    color: CelestinaTheme.accentSoft
                    radius: CelestinaTheme.radiusXs
                }
            }

            // The viewport below a short document. It is inside the Flickable
            // rather than behind it because a scrollable Flickable takes every
            // press it receives, so nothing under it would ever see the click.
            // Behind the widget: a click on a line still belongs to the text.
            // Stacking order this relies on: declared before `body`, so the
            // TextEdit is above it and takes every press on painted lines.
            MouseArea {
                width: Math.max(scroller.width, scroller.contentWidth)
                height: Math.max(scroller.height, scroller.contentHeight)
                onClicked: {
                    body.cursorPosition = body.length
                    body.forceActiveFocus()
                }
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
                // An edit can change the brackets beside a caret that did not
                // move (Delete, an undo, a newly adopted text), so the pair is
                // re-placed here too; beside no bracket that costs two
                // character reads.
                onTextChanged: {
                    root.session.applyText(text)
                    placeBrackets()
                }
                onCursorRectangleChanged: scroller.revealCursor(cursorRectangle)
                // The widget knows where its caret is as a UTF-16 offset; only
                // the document can say which line and column that is.
                onCursorPositionChanged: {
                    root.session.setCaret(cursorPosition)
                    placeBrackets()
                }
                // Rewrapping or resizing the text moves every character.
                onContentSizeChanged: placeBrackets()

                // The bracket beside the caret and its partner, as boxes in
                // this widget's coordinates; empty when there is no pair. The
                // highlighter finds the pair by reading the document, so
                // nothing here edits the text or its undo history.
                property var bracketBoxes: []

                function placeBrackets() {
                    const pair = colouring.matchBracket(cursorPosition)
                    const boxes = []
                    for (let i = 0; i < pair.length; ++i) {
                        const at = positionToRectangle(pair[i])
                        boxes.push(Qt.rect(at.x, at.y,
                                           bracketMetrics.advanceWidth(getText(pair[i], pair[i] + 1)),
                                           at.height))
                    }
                    bracketBoxes = boxes
                }

                FontMetrics {
                    id: bracketMetrics
                    font: body.font
                }

                // The editing verbs, on the right button. The widget itself
                // ignores that button, so the handler sees every press; the
                // menu opens on release, the way every menu in the suite does,
                // so the press that opened it can never also pick an item.
                TapHandler {
                    acceptedButtons: Qt.RightButton
                    onTapped: function(eventPoint) {
                        body.forceActiveFocus()
                        const point = body.mapToItem(root, eventPoint.position.x,
                                                     eventPoint.position.y)
                        editMenu.popup(root, point)
                    }
                }

                // Colour without touching the text. A QSyntaxHighlighter applies
                // formats to the document's blocks and leaves the characters
                // alone, so what this widget reports back is still exactly the
                // core's projection — anything that rewrote the text as markup
                // would break the reconciliation instead. The definition is
                // KSyntaxHighlighting's for the file's name (or first line);
                // each colour is a theme token, one per role.
                SyntaxHighlighter {
                    id: colouring
                    target: body.textDocument
                    fileName: root.session.syntaxFileName
                    keywordColor: CelestinaTheme.codeKeyword
                    functionColor: CelestinaTheme.glyphAccentBlue
                    typeColor: CelestinaTheme.glyphAccentCyan
                    builtInColor: CelestinaTheme.glyphAccentViolet
                    attributeColor: CelestinaTheme.glyphAccentCoral
                    stringColor: CelestinaTheme.codeString
                    escapeColor: CelestinaTheme.glyphAccentAmber
                    numberColor: CelestinaTheme.codeNumber
                    commentColor: CelestinaTheme.codeComment
                    operatorColor: CelestinaTheme.textMuted
                    warningColor: CelestinaTheme.warning
                    errorColor: CelestinaTheme.danger
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

    // The editing surface's context menu. Cut and copy need a selection and
    // paste needs a clipboard with text in it; the widget knows both, so the
    // items enable themselves from it rather than from a guess.
    GlassContextMenu {
        id: editMenu
        backdropSource: page

        GlassMenuItem {
            text: qsTr("Cortar")
            icon.name: "scissors"
            icon.source: CelestinaTheme.fallbackIcon("scissors")
            enabled: body.selectedText.length > 0
            onTriggered: body.cut()
        }
        GlassMenuItem {
            text: qsTr("Copiar")
            icon.name: "copy"
            icon.source: CelestinaTheme.fallbackIcon("copy")
            enabled: body.selectedText.length > 0
            onTriggered: body.copy()
        }
        GlassMenuItem {
            text: qsTr("Pegar")
            icon.name: "clipboard-paste"
            icon.source: CelestinaTheme.fallbackIcon("clipboard-paste")
            enabled: body.canPaste
            onTriggered: body.paste()
        }
        GlassMenuItem {
            text: qsTr("Seleccionar todo")
            icon.name: "files"
            icon.source: CelestinaTheme.fallbackIcon("files")
            enabled: body.length > 0
            onTriggered: body.selectAll()
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

    // The documents this window could pick up again. Asked for again whenever
    // the empty state appears rather than held: another window may have opened
    // something since, and a stale history is the one thing a history must not
    // be. The session's worker reads it and the list arrives through the
    // property, so a path on a dead mount never stalls this window.
    readonly property var recentPaths: root.session.recentDocuments
    function refreshRecent() {
        root.session.refreshRecent()
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

    // A button label with a leading glyph, inked like the shared button's own
    // text so the two empty-state verbs read as the same control with an icon.
    component VerbLabel: Row {
        id: verb
        required property CelestinaButton owner
        required property string glyph
        readonly property bool onAccent: verb.owner.role === CelestinaButton.Primary
        spacing: CelestinaTheme.spaceXs

        CelestinaIcon {
            anchors.verticalCenter: parent.verticalCenter
            name: verb.glyph
            tone: !verb.owner.enabled ? CelestinaIcon.Secondary
                  : verb.onAccent ? CelestinaIcon.OnAccent : CelestinaIcon.Primary
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: verb.owner.text
            textFormat: Text.PlainText
            font: verb.owner.font
            color: !verb.owner.enabled ? CelestinaTheme.textMuted
                   : verb.onAccent ? CelestinaTheme.accentInk : CelestinaTheme.text
        }
    }

    // Nothing open: say so, offer the way in, and say what went wrong if
    // something did. An editor with no document and no button is a dead end.
    Item {
        id: emptyState
        anchors.fill: parent
        visible: !root.session.active

        // The invitation keeps to its 420 pixel column; the recents box takes
        // the same width so the two read as one centred stack.
        readonly property real columnWidth: Math.min(420, emptyState.width - CelestinaTheme.space3xl)

        // Group geometry. The area ends where the page box ends, above the footer.
        readonly property real areaHeight: Math.max(0, footer.y - CelestinaTheme.spaceSm)
        readonly property real boxWanted: recentsBox.padding * 2 + recentsLabel.height
                                          + CelestinaTheme.spaceXs + recentsList.contentHeight
        readonly property real boxRoom: Math.max(0, emptyState.areaHeight - CelestinaTheme.space3xl
                                                    - invitation.height - CelestinaTheme.spaceLg)
        // Room for the label and at least one row, or no box at all.
        readonly property bool hasBox: root.recentPaths.length > 0
                                       && emptyState.boxRoom >= recentsBox.padding * 2
                                          + recentsLabel.height + CelestinaTheme.spaceXs
                                          + CelestinaTheme.controlHeight
        readonly property real boxHeight: Math.min(emptyState.boxWanted, emptyState.boxRoom)
        readonly property bool fits: !emptyState.hasBox || emptyState.boxWanted <= emptyState.boxRoom
        readonly property real groupHeight: invitation.height + (emptyState.hasBox
                                            ? CelestinaTheme.spaceLg + emptyState.boxHeight : 0)

        Column {
            id: invitation
            anchors.horizontalCenter: parent.horizontalCenter
            width: emptyState.columnWidth
            spacing: CelestinaTheme.spaceSm
            // The invitation and the recents box are one group. It is centred
            // in the empty area while it fits, and sits `space3xl` from the top
            // only when the box has to take all the height there is. A move
            // (recents arrive after start) is a glide, never a jump.
            y: emptyState.fits
               ? Math.round((emptyState.areaHeight - emptyState.groupHeight) / 2)
               : CelestinaTheme.space3xl

            // The first placement is instant: the glide is enabled only once
            // the area has a height and one frame has laid the group out.
            property bool placed: false
            Connections {
                target: emptyState
                function onAreaHeightChanged() {
                    if (emptyState.areaHeight > 0 && !invitation.placed)
                        Qt.callLater(function() { invitation.placed = true })
                }
            }
            Component.onCompleted: if (emptyState.areaHeight > 0)
                Qt.callLater(function() { invitation.placed = true })

            Behavior on y {
                enabled: invitation.placed
                NumberAnimation {
                    duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionNormal
                    easing.type: CelestinaTheme.easeStandard
                }
            }

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

                    // The page's two verbs keep their words: a glyph in front says
                    // what each opens, the text says it in the user's language.
                    CelestinaButton {
                        id: openButton
                        text: "Abrir archivo…"
                        role: CelestinaButton.Primary
                        enabled: !root.session.busy
                        onClicked: openDialog.open()
                        contentItem: VerbLabel { owner: openButton; glyph: "folder-open" }
                    }

                    CelestinaButton {
                        id: newButton
                        text: "Documento nuevo"
                        enabled: !root.session.busy
                        onClicked: root.session.newDocument()
                        contentItem: VerbLabel { owner: newButton; glyph: "file-plus" }
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

        // Recent documents, if there are any. A new tab that remembers what you
        // were working on beats a new tab that makes you go find it again. The
        // box is an opaque `card`, like the document page, so the names never sit
        // over the glass; it takes the height left under the buttons and the list
        // scrolls inside it, which is what lets the history be long.
        CelestinaSurface {
            id: recentsBox
            role: CelestinaSurface.Panel
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: invitation.bottom
            anchors.topMargin: CelestinaTheme.spaceLg
            height: emptyState.boxHeight
            width: emptyState.columnWidth
            // Inset from the rounded corners by the surface's own padding, so
            // neither the label nor a row's text meets the curve.
            padding: CelestinaTheme.spaceLg
            visible: emptyState.hasBox

            CelestinaSectionLabel {
                id: recentsLabel
                text: "Recientes"
            }

            ListView {
                id: recentsList
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: recentsLabel.bottom
                anchors.topMargin: CelestinaTheme.spaceXs
                anchors.bottom: parent.bottom
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                model: root.recentPaths
                // Every row exists, so Tab can reach one that is out of view.
                cacheBuffer: CelestinaTheme.controlHeight * 64

                CelestinaWheelScroll { view: recentsList }

                CelestinaScrollBar {
                    surface: recentsList
                    anchors.right: recentsList.right
                    anchors.top: recentsList.top
                    anchors.bottom: recentsList.bottom
                }

                // The shared button rather than a hand-rolled row: it brings
                // Tab focus, Return and Space, the focus ring and the pressed
                // fill that a plain rectangle with a MouseArea had none of.
                // Ghost, so at rest it is the bare text the row always was.
                delegate: CelestinaButton {
                    id: entry
                    required property string modelData
                    required property int index

                    width: recentsList.width
                    height: CelestinaTheme.controlHeight
                    role: CelestinaButton.Ghost
                    leftPadding: CelestinaTheme.spaceSm
                    rightPadding: CelestinaTheme.spaceLg
                    // The name reads first; the folder is there to tell two
                    // files of the same name apart.
                    text: root.baseName(entry.modelData)
                    onClicked: root.session.openPath(entry.modelData)
                    onActiveFocusChanged: if (entry.activeFocus)
                        recentsList.positionViewAtIndex(entry.index, ListView.Contain)

                    Accessible.name: "Abrir " + root.baseName(entry.modelData)
                    Accessible.description: entry.modelData

                    // Only for the cursor: a Control has no `cursorShape`, so
                    // this handler is the one way to get the hand. It reads
                    // nothing else — the hover fill is the button's own.
                    HoverHandler { cursorShape: Qt.PointingHandCursor }

                    contentItem: Text {
                        text: entry.text
                        textFormat: Text.PlainText
                        elide: Text.ElideMiddle
                        horizontalAlignment: Text.AlignLeft
                        verticalAlignment: Text.AlignVCenter
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                    }
                }
            }
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
