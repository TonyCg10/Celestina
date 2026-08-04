import QtQuick
import QtQuick.Window
import org.celestina.siderita 1.0

    // ── Embedded Grafita editor (spacebar on text) ───────────────────
    // A real editor, not a preview: it types, saves, undoes and refuses to
    // vanish with unsaved work. It occupies nearly the whole folder surface
    // and hands the keyboard back to the view when it closes.
    //
    // The text widget does not own the text. It shows `grafita-core`'s
    // line-feed projection and reports its whole content back on every change;
    // the core turns that into the one splice it represents, which is why a
    // CRLF file survives being edited here.
CelestinaModalLayer {
    id: editorLayer

    property var editor      // GrafitaEditor
    property var owner       // FolderView root: focus return and sizing
    property var backdrop    // mainPanel: what the glass samples
    // GrafitaPreferences: the text size the reader chose, shared with Grafita's
    // own window through the file it stores.
    property var reading

    // Declared inside the folder view, but it is a window-level surface: left
    // as a child of the view it stopped at the view's edge, leaving the sidebar
    // lit, clickable and outside the scrim. Reparenting to the window's content
    // item covers the whole window without the folder view having to hand an
    // overlay parent down.
    parent: editorLayer.Window.contentItem
    anchors.fill: parent
    z: 75
    shown: editor.active
    // A document with unsaved work never disappears by accident: both Escape
    // and a click outside go through the guarded close, never around it.
    dismissOnEscape: false
    dismissOnOutsideClick: false
    onDismissRequested: editorLayer.editor.requestClose()

    // Whatever is stored right now, not whatever was stored when this folder
    // view was built: Grafita may be open beside Siderita and may have moved
    // the size since.
    onShownChanged: if (editorLayer.shown) editorLayer.reading.reload()

    Connections {
        target: editorLayer.editor

        // The document replaced its own text — an open, an undo or a redo.
        // Assigning it back is not an edit: the core recognises its projection.
        function onDocumentReset(text, caret) {
            body.text = text
            body.cursorPosition = Math.min(caret, body.length)
            if (!editorLayer.editor.closePrompt)
                body.forceActiveFocus()
        }

        function onClosed() {
            editorLayer.owner.focusView()
        }
    }

    GlassCard {
        id: card
        anchors.centerIn: parent
        // Sized against the layer — that is, the window — now that the editor
        // is a window-level surface rather than a guest of the folder view.
        width: Math.max(360, editorLayer.width - CelestinaTheme.space3xl)
        height: Math.max(280, editorLayer.height - CelestinaTheme.space2xl)
        backdropSource: editorLayer.backdrop
        Accessible.role: Accessible.Dialog
        Accessible.name: "Editar " + editorLayer.editor.name

        // Clicks on the card never reach the dismissing backdrop.
        MouseArea { anchors.fill: parent }

        // ── Everything the document owns, disabled while the guarded-close
        // question is up so the surface underneath cannot be acted on.
        Item {
            id: sheet
            anchors.fill: parent
            enabled: !editorLayer.editor.closePrompt

            Text {
                id: heading
                x: CelestinaTheme.spaceLg
                y: CelestinaTheme.spaceMd
                width: parent.width - CelestinaTheme.space2xl
                elide: Text.ElideMiddle
                text: editorLayer.editor.dirty
                      ? editorLayer.editor.name + " •"
                      : editorLayer.editor.name
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.weight: CelestinaTheme.weightDemiBold

                // The bullet says "unsaved" to the eye only. Assistive
                // technology is told in words instead of being read a
                // punctuation mark.
                Accessible.role: Accessible.StaticText
                Accessible.name: editorLayer.editor.dirty
                                 ? editorLayer.editor.name + ", sin guardar"
                                 : editorLayer.editor.name
            }

            // The encoding is deliberately not shown, as in Grafita's own
            // window: the editor only opens what it can write back byte for
            // byte, so the label's answer never changes and never asks for an
            // action. The encoding itself is still the document's, and the
            // core still preserves it.

            // A refusal or a disagreement with the file on disk. Both are the
            // user's to act on, so neither is a transient toast.
            Text {
                id: notice
                x: CelestinaTheme.spaceLg
                y: heading.y + heading.height + CelestinaTheme.spaceXs
                width: parent.width - CelestinaTheme.space2xl
                wrapMode: Text.WordWrap
                visible: text.length > 0
                text: editorLayer.editor.errorText.length > 0
                      ? editorLayer.editor.errorText
                      : editorLayer.editor.conflictText
                color: editorLayer.editor.errorText.length > 0
                       ? CelestinaTheme.danger : CelestinaTheme.warning
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption

                // Red versus amber is the whole visual difference between a
                // refusal and a conflict, which colour alone must not carry.
                Accessible.role: Accessible.AlertMessage
                Accessible.name: (editorLayer.editor.errorText.length > 0
                                  ? "Error: " : "Aviso: ") + notice.text
            }

            Rectangle {
                id: page
                x: CelestinaTheme.spaceLg
                y: notice.y + (notice.visible ? notice.height + CelestinaTheme.spaceXs : 0)
                   + CelestinaTheme.spaceSm
                width: parent.width - CelestinaTheme.space2xl
                height: footer.y - y - CelestinaTheme.spaceSm
                color: CelestinaTheme.inputFill
                radius: CelestinaTheme.radiusInput
                border.width: CelestinaTheme.borderHairline
                border.color: CelestinaTheme.inputBorder

                // No focus ring here, deliberately. The ring is reserved for
                // keyboard focus, and a bare `TextEdit` is a TextInput template
                // that exposes no `focusReason` — the signal CelestinaTextField
                // uses to tell Tab from a click. Painting the ring on plain
                // `activeFocus` would light it up on every click, which is what
                // the contract forbids. For an editing surface the caret is the
                // focus affordance, and it is already there.

                // Numbered lines, and the text set in from the frame rather
                // than run against it — the same surface Grafita's own window
                // shows, because it is the same document being edited.
                CelestinaLineGutter {
                    id: gutter
                    anchors.left: parent.left
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
                    anchors.left: gutter.right
                    anchors.leftMargin: CelestinaTheme.spaceMd
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    anchors.rightMargin: CelestinaTheme.spaceMd
                    anchors.topMargin: CelestinaTheme.spaceMd
                    anchors.bottomMargin: CelestinaTheme.spaceMd
                    clip: true
                    contentWidth: width
                    contentHeight: body.paintedHeight
                    boundsBehavior: Flickable.StopAtBounds

                    // Ctrl and the wheel resize the text instead of scrolling
                    // it, accumulated to a full notch so a touchpad moves one
                    // step at a time rather than sweeping the whole range.
                    WheelHandler {
                        property real pending: 0
                        readonly property real notch: 120

                        acceptedModifiers: Qt.ControlModifier
                        onWheel: function(event) {
                            pending += event.angleDelta.y
                            while (pending >= notch) {
                                pending -= notch
                                editorLayer.reading.enlargeText()
                            }
                            while (pending <= -notch) {
                                pending += notch
                                editorLayer.reading.shrinkText()
                            }
                        }
                    }

                    // Keep the caret on screen without animating the viewport,
                    // which would be motion the user did not ask for.
                    function revealCursor(rectangle) {
                        if (rectangle.y < contentY)
                            contentY = rectangle.y
                        else if (rectangle.y + rectangle.height > contentY + height)
                            contentY = rectangle.y + rectangle.height - height
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
                        font.pixelSize: editorLayer.reading.fontSize

                        Accessible.role: Accessible.EditableText
                        Accessible.name: "Contenido de " + editorLayer.editor.name

                        // The core is the document; this reports what the
                        // widget now holds and lets the core work out the edit.
                        onTextChanged: editorLayer.editor.applyText(text)
                        onCursorRectangleChanged: scroller.revealCursor(cursorRectangle)
                        // The widget knows its caret as a UTF-16 offset; only
                        // the document can say which line and column that is.
                        onCursorPositionChanged: editorLayer.editor.setCaret(cursorPosition)

                        // Undo, redo and save are the document's, not the
                        // widget's: intercepted before Qt's own text history,
                        // which knows nothing about savepoints or terminators.
                        Keys.onPressed: function(event) {
                            const control = event.modifiers & Qt.ControlModifier
                            const shift = event.modifiers & Qt.ShiftModifier
                            if (event.key === Qt.Key_Escape) {
                                editorLayer.editor.requestClose()
                                event.accepted = true
                            } else if (control && event.key === Qt.Key_S) {
                                editorLayer.editor.save()
                                event.accepted = true
                            } else if (control && !shift && event.key === Qt.Key_Z) {
                                editorLayer.editor.undo()
                                event.accepted = true
                            } else if (control && (event.key === Qt.Key_Y
                                                   || (shift && event.key === Qt.Key_Z))) {
                                editorLayer.editor.redo()
                                event.accepted = true
                            } else if (control && (event.key === Qt.Key_Plus
                                                   || event.key === Qt.Key_Equal)) {
                                // The same size, moved from the same surface
                                // the reader is in. A stored size that could
                                // only be changed by leaving for Grafita would
                                // be half a preference.
                                editorLayer.reading.enlargeText()
                                event.accepted = true
                            } else if (control && (event.key === Qt.Key_Minus
                                                   || event.key === Qt.Key_Underscore)) {
                                editorLayer.reading.shrinkText()
                                event.accepted = true
                            }
                        }
                    }
                }

                // Over the viewport's edges rather than beside them, and only
                // while there is something to scroll.
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

            // Where the caret is, in the document's own counting: the column
            // is characters, so an accented letter is one column.
            Text {
                id: caret
                x: CelestinaTheme.spaceLg
                anchors.verticalCenter: footer.verticalCenter
                visible: editorLayer.editor.active
                text: "Ln " + editorLayer.editor.caretLine
                      + ", Col " + editorLayer.editor.caretColumn
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.monoFamily
                font.pixelSize: CelestinaTheme.fontCaption

                Accessible.role: Accessible.StaticText
                Accessible.name: "Line " + editorLayer.editor.caretLine
                                 + ", column " + editorLayer.editor.caretColumn
            }

            Text {
                id: status
                x: caret.visible ? caret.x + caret.width + CelestinaTheme.spaceLg
                                 : CelestinaTheme.spaceLg
                anchors.verticalCenter: footer.verticalCenter
                width: parent.width - footer.width - status.x - CelestinaTheme.spaceLg
                elide: Text.ElideRight
                text: editorLayer.editor.busy ? "Trabajando…" : editorLayer.editor.statusText
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption

                Accessible.role: Accessible.StaticText
                Accessible.name: status.text
                Accessible.ignored: status.text.length === 0
            }

            Row {
                id: footer
                anchors.right: parent.right
                anchors.rightMargin: CelestinaTheme.spaceLg
                anchors.bottom: parent.bottom
                anchors.bottomMargin: CelestinaTheme.spaceMd
                spacing: CelestinaTheme.spaceSm

                CelestinaButton {
                    text: "Deshacer"
                    enabled: editorLayer.editor.canUndo
                    onClicked: editorLayer.editor.undo()
                }
                CelestinaButton {
                    text: "Rehacer"
                    enabled: editorLayer.editor.canRedo
                    onClicked: editorLayer.editor.redo()
                }
                CelestinaButton {
                    text: "Cerrar"
                    onClicked: editorLayer.editor.requestClose()
                }
                CelestinaButton {
                    text: "Guardar"
                    role: CelestinaButton.Primary
                    enabled: editorLayer.editor.dirty && !editorLayer.editor.busy
                    onClicked: editorLayer.editor.save()
                }
            }
        }

        // ── Guarded close ────────────────────────────────────────────────
        // A dirty document does not close on a keystroke. The question owns
        // the focus while it is up and every answer is explicit.
        Rectangle {
            id: guard
            anchors.fill: parent
            visible: editorLayer.editor.closePrompt
            color: CelestinaTheme.scrim
            radius: CelestinaTheme.radiusMd

            MouseArea { anchors.fill: parent }

            onVisibleChanged: if (visible) keepButton.forceActiveFocus()

            Item {
                anchors.centerIn: parent
                width: Math.min(420, card.width - CelestinaTheme.space2xl)
                height: question.height + guardButtons.height + CelestinaTheme.space2xl

                Accessible.role: Accessible.Dialog
                Accessible.name: "Cambios sin guardar"

                Text {
                    id: question
                    width: parent.width
                    wrapMode: Text.WordWrap
                    horizontalAlignment: Text.AlignHCenter
                    text: "«" + editorLayer.editor.name + "» tiene cambios sin guardar."
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                }

                Row {
                    id: guardButtons
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.top: question.bottom
                    anchors.topMargin: CelestinaTheme.spaceLg
                    spacing: CelestinaTheme.spaceSm

                    CelestinaButton {
                        text: "Cancelar"
                        onClicked: editorLayer.editor.cancelClose()
                    }
                    CelestinaButton {
                        text: "Descartar"
                        onClicked: editorLayer.editor.discardAndClose()
                    }
                    CelestinaButton {
                        id: keepButton
                        text: "Guardar"
                        role: CelestinaButton.Primary
                        onClicked: editorLayer.editor.saveAndClose()
                    }
                }
            }
        }
    }
}
