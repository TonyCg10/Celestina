import QtQuick
import QtQuick.Controls
import QtQuick.Window
import QtTest
import CelestinaStyle

TestCase {
    id: testCase

    name: "CelestinaModalLayer"
    when: testWindow.visible

    property int lowerClicks: 0
    property bool lowerDragged: false
    property int modalClicks: 0
    property bool dismissed: false

    Window {
        id: testWindow

        width: 480
        height: 320
        visible: true

        Item {
            id: scene

            anchors.fill: parent

            // The surface a modal covers is not passive. It is modelled here as
            // what actually sits under one in the suite: a list whose rows carry
            // both a MouseArea and a DragHandler, because a handler keeps its
            // passive grab under an item that merely swallows clicks.
            ListView {
                id: lowerList

                anchors.fill: parent
                model: 8
                clip: true

                delegate: Item {
                    width: lowerList.width
                    height: 40

                    MouseArea {
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        hoverEnabled: true
                        onClicked: testCase.lowerClicks += 1
                    }

                    DragHandler {
                        target: null
                        dragThreshold: 8
                        onActiveChanged: if (active) testCase.lowerDragged = true
                    }
                }
            }

            // A field of the covered surface lying right under the card's
            // empty part, as a search field under a dialog does.
            TextField {
                id: lowerField
                x: 240
                y: 40
                width: 180
                text: "under the dialog"
            }

            Button {
                id: lowerButton
                x: 12
                y: 12
                text: "Lower surface"
            }

            CelestinaModalLayer {
                id: modal

                anchors.fill: parent
                z: 10
                dismissOnOutsideClick: true
                onDismissRequested: testCase.dismissed = true

                Column {
                    anchors.centerIn: parent
                    spacing: CelestinaTheme.spaceSm

                    TextField {
                        id: firstField
                        width: 180
                        text: "first"
                    }

                    // The suite's own field, the one every prompt carries: a
                    // new-folder name, a password, an archive name.
                    CelestinaTextField {
                        id: promptField
                        width: 180
                        activeFocusOnTab: false
                        text: "a folder name long enough to sweep"
                    }

                    CheckBox {
                        id: middleCheck
                        text: "middle"
                    }

                    ListView {
                        id: middleList
                        width: 180
                        height: 48
                        activeFocusOnTab: true
                        model: ["one", "two"]
                        delegate: Text {
                            required property string modelData
                            text: modelData
                        }
                    }

                    Button {
                        id: lastButton
                        text: "last"
                        onClicked: testCase.modalClicks += 1
                    }

                    // A listing that fills in after the dialog is up, like the
                    // usage section of the properties dialog: its rows arrive
                    // from a scan, and the list puts a cursor on the first.
                    CelestinaUsageList {
                        id: lateList
                        width: 180
                        height: 60
                        usageRows: []
                        toneColors: ({})
                    }
                }

                // Selectable text of the kind a preview carries.
                TextEdit {
                    id: previewText
                    x: 4
                    y: 200
                    width: 140
                    activeFocusOnTab: false
                    readOnly: true
                    selectByMouse: true
                    text: "a preview line to select"
                }

                // A field partly behind a pane (a control that is not a
                // button) and partly behind a flickable: a press on either is
                // theirs, never the field's.
                TextField {
                    id: paneField
                    x: 4
                    y: 250
                    width: 140
                    activeFocusOnTab: false
                    text: "behind a pane and a list"
                }

                Pane {
                    id: coveringPane
                    x: 4
                    y: 250
                    width: 70
                    height: 40
                }

                Flickable {
                    id: coveringFlickable
                    x: 74
                    y: 250
                    width: 70
                    height: 40
                    contentHeight: 120
                }

                // The empty part of a dialog card. A card carries a MouseArea
                // of its own so a click on it does not dismiss — and that item
                // grabber, not the layer, is what took the press, which is
                // exactly the path a sweep used to escape through.
                Item {
                    id: emptyCardArea
                    x: 240
                    y: 40
                    width: 180
                    height: 200

                    MouseArea { anchors.fill: parent }
                }
            }
        }
    }

    function focusLowerSurface() {
        lowerButton.forceActiveFocus(Qt.TabFocusReason)
        tryCompare(lowerButton, "activeFocus", true)
    }

    function openWithConsumerFocus() {
        focusLowerSurface()
        modal.shown = true
        compare(modal.shown, true)
        compare(modal.previousFocusItem, lowerButton)
        tryCompare(modal, "visible", true)
        tryCompare(modal, "activeFocus", true)
    }

    function init() {
        CelestinaTheme.reducedMotion = true
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        modal.shown = false
        modal.dismissOnOutsideClick = true
        dismissed = false
        lowerClicks = 0
        lowerDragged = false
        modalClicks = 0
        wait(0)
        focusLowerSurface()
    }

    function cleanup() {
        modal.shown = false
        wait(0)
    }

    // The layer takes the focus on show; no control lights a ring nobody
    // asked for. Tab travel then starts at the first control.
    function test_takes_focus_itself_without_entering_a_control() {
        openWithConsumerFocus()
        verify(modal.activeFocus)
        verify(!firstField.activeFocus)
        verify(!lowerButton.activeFocus)
        keyClick(Qt.Key_Tab)
        tryCompare(firstField, "activeFocus", true)
    }

    function test_preserves_initial_focus_and_restores_it() {
        CelestinaTheme.reducedMotion = false
        openWithConsumerFocus()
        modal.shown = false
        tryCompare(modal, "visible", false)
        tryCompare(lowerButton, "activeFocus", true)
    }

    function test_tab_and_backtab_stay_inside() {
        openWithConsumerFocus()

        keyClick(Qt.Key_Tab)
        tryCompare(firstField, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(middleCheck, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(middleList, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(lastButton, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(lateList, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(firstField, "activeFocus", true)

        keyClick(Qt.Key_Backtab)
        tryCompare(lateList, "activeFocus", true)
        verify(!lowerButton.activeFocus)
    }

    function test_escape_and_pointer_blocking() {
        openWithConsumerFocus()
        modal.dismissOnOutsideClick = false
        mouseClick(lastButton, lastButton.width / 2,
                   lastButton.height / 2, Qt.LeftButton)
        compare(modalClicks, 1)
        compare(dismissed, false)

        mouseClick(scene, 4, 4, Qt.LeftButton)
        compare(lowerClicks, 0)
        compare(dismissed, false)

        keyClick(Qt.Key_Escape)
        compare(dismissed, true)
    }

    // A modal that only swallows clicks is not blocking: sweeping over its own
    // empty space still drove the drag handler underneath.
    function test_pointer_blocking_covers_drags() {
        openWithConsumerFocus()

        // The sweep leaves the card, which is the shape that used to escape:
        // inside its own bounds the layer's MouseArea kept the grab, but a
        // pointer on its way out reached the handler below.
        mousePress(emptyCardArea, 20, 20, Qt.LeftButton)
        mouseMove(emptyCardArea, 30, 26)
        mouseMove(emptyCardArea, 60, 40)
        mouseMove(emptyCardArea, 120, 70)
        mouseMove(emptyCardArea, 200, 140)
        mouseRelease(emptyCardArea, 200, 140, Qt.LeftButton)

        compare(lowerDragged, false)
        compare(lowerClicks, 0)
    }

    // ...and the dialog's own controls keep theirs: the shield claims the drag
    // on the press, so a text sweep inside the modal must still select.
    function test_drag_claim_leaves_the_dialog_usable() {
        openWithConsumerFocus()

        mouseClick(lastButton, lastButton.width / 2,
                   lastButton.height / 2, Qt.LeftButton)
        compare(modalClicks, 1)

        firstField.select(0, 0)
        mousePress(firstField, 8, firstField.height / 2, Qt.LeftButton)
        mouseMove(firstField, 60, firstField.height / 2)
        mouseMove(firstField, 120, firstField.height / 2)
        mouseRelease(firstField, 120, firstField.height / 2, Qt.LeftButton)
        verify(firstField.selectedText.length > 0)
        compare(lowerDragged, false)
    }

    // A press and a few pixels at a time, the way a hand actually moves. The
    // fast sweep above jumps 60 px per event and so passed while a slow one
    // selected nothing: the shield's zero-threshold drag took the grab on the
    // first pixel, before the field's own start-drag distance let it keep it.
    function slowSweep(item, fromX, toX, y, step) {
        mousePress(item, fromX, y, Qt.LeftButton)
        for (let x = fromX + step; x <= toX; x += step)
            mouseMove(item, x, y)
        mouseRelease(item, toX, y, Qt.LeftButton)
    }

    function test_slow_sweep_selects_in_a_modal_field() {
        openWithConsumerFocus()
        promptField.select(0, 0)
        const y = promptField.height / 2
        slowSweep(promptField, 8, 120, y, 2)
        compare(promptField.selectionStart, promptField.positionAt(8, y))
        compare(promptField.selectionEnd, promptField.positionAt(120, y))
        verify(promptField.selectedText.length > 10,
               "selected only '" + promptField.selectedText + "'")
        // ...and it took the focus, as a click on it does.
        verify(promptField.activeFocus)
        compare(lowerDragged, false)
        compare(lowerClicks, 0)
        compare(dismissed, false)
    }

    function test_slow_sweep_selects_in_a_text_edit() {
        openWithConsumerFocus()
        previewText.select(0, 0)
        slowSweep(previewText, 2, 100, previewText.height / 2, 2)
        verify(previewText.selectedText.length > 5,
               "selected only '" + previewText.selectedText + "'")
        compare(lowerDragged, false)
    }

    // The selection is carried only for a field that took the press. A sweep
    // that starts on the card's empty part selects nothing: not in the
    // dialog's field that part lies over, not in a field it crosses, and
    // never in the covered field under it all.
    function test_a_sweep_from_the_card_selects_nowhere() {
        openWithConsumerFocus()
        const pressed = emptyCardArea.mapToItem(firstField, 2, 10)
        verify(firstField.contains(pressed), "the card part must lie over the field")
        firstField.select(0, 0)
        lowerField.select(0, 0)
        mousePress(emptyCardArea, 2, 10, Qt.LeftButton)
        for (let i = 1; i <= 60; ++i)
            mouseMove(emptyCardArea, 2 + i * 2, 10)
        mouseRelease(emptyCardArea, 122, 10, Qt.LeftButton)
        compare(firstField.selectedText, "")
        compare(lowerField.selectedText, "")
        verify(!firstField.activeFocus)
        verify(!lowerField.activeFocus)
        compare(lowerDragged, false)

        promptField.select(0, 0)
        const start = emptyCardArea.mapFromItem(promptField, 0, promptField.height / 2)
        mousePress(emptyCardArea, 2, 10, Qt.LeftButton)
        for (let i = 1; i <= 40; ++i)
            mouseMove(emptyCardArea, 2 - i * 2, 10 + (start.y - 10) * i / 40)
        mouseRelease(emptyCardArea, -78, start.y, Qt.LeftButton)
        compare(promptField.selectedText, "")
        compare(lowerDragged, false)
    }

    function test_slow_sweep_selects_in_a_plain_field_too() {
        openWithConsumerFocus()
        firstField.select(0, 0)
        slowSweep(firstField, 4, 40, firstField.height / 2, 1)
        verify(firstField.selectedText.length > 1,
               "selected only '" + firstField.selectedText + "'")
        compare(lowerDragged, false)
    }

    // The selection may run out of the card and over what the dialog covers:
    // the field keeps the pointer and the handler below still never moves.
    function test_slow_sweep_out_of_a_field_does_not_reach_below() {
        openWithConsumerFocus()
        promptField.select(0, 0)
        const y = promptField.height / 2
        const start = promptField.positionAt(8, y)
        mousePress(promptField, 8, y, Qt.LeftButton)
        for (let i = 1; i <= 120; ++i)
            mouseMove(promptField, 8 + i * 2, y + i * 2)
        mouseRelease(promptField, 248, y + 240, Qt.LeftButton)
        // The field scrolls as the selection runs to its end, so the start
        // is where the press was before it moved.
        compare(promptField.selectionStart, start)
        compare(promptField.selectionEnd, promptField.length)
        compare(lowerDragged, false)
        compare(lowerClicks, 0)
        compare(dismissed, false)
    }

    // A press on a pane or a flickable over a field is theirs: the drag moves
    // neither the field's selection nor its focus.
    function test_a_sweep_from_a_control_over_a_field_selects_nowhere() {
        openWithConsumerFocus()
        for (const cover of [coveringPane, coveringFlickable]) {
            paneField.select(0, 0)
            paneField.cursorPosition = 0
            const y = cover.height / 2
            mousePress(cover, 4, y, Qt.LeftButton)
            for (let x = 6; x <= 60; x += 2)
                mouseMove(cover, x, y)
            mouseRelease(cover, 60, y, Qt.LeftButton)
            compare(paneField.selectedText, "", "selected through " + cover)
            verify(!paneField.activeFocus, "focused through " + cover)
        }
        compare(lowerDragged, false)
    }

    // A dialog accepted mid-drag: its field stops following the pointer and
    // the release that follows gives it no focus.
    function test_a_dialog_closed_mid_drag_gives_its_field_nothing() {
        openWithConsumerFocus()
        promptField.select(0, 0)
        const y = promptField.height / 2
        mousePress(promptField, 8, y, Qt.LeftButton)
        for (let x = 10; x <= 60; x += 2)
            mouseMove(promptField, x, y)
        const selected = promptField.selectionEnd
        verify(selected > 0)
        modal.shown = false
        tryCompare(modal, "visible", false)
        for (let x = 62; x <= 140; x += 2)
            mouseMove(promptField, x, y)
        mouseRelease(promptField, 140, y, Qt.LeftButton)
        compare(promptField.selectionEnd, selected)
        verify(!promptField.activeFocus)
        // Not even the focus flag, which it would carry into the next open.
        verify(!promptField.focus)
    }

    // The shield's own reason to exist, at the same slow pace: a sweep from
    // the card's empty part never drives the drag handler underneath.
    function test_slow_sweep_from_the_card_does_not_reach_below() {
        openWithConsumerFocus()
        mousePress(emptyCardArea, 20, 20, Qt.LeftButton)
        for (let i = 1; i <= 100; ++i)
            mouseMove(emptyCardArea, 20 + i * 2, 20 + i * 2)
        mouseRelease(emptyCardArea, 220, 220, Qt.LeftButton)
        compare(lowerDragged, false)
        compare(lowerClicks, 0)
        compare(dismissed, false)
    }

    // The picture that fades is the last complete one: the stage renders
    // through a snapshot that is live while shown and frozen from the moment
    // `shown` drops, whatever the dialog's content does to itself on close.
    function test_exit_fade_is_a_frozen_snapshot() {
        CelestinaTheme.reducedMotion = false
        openWithConsumerFocus()
        tryCompare(modal, "opacity", 1)
        verify(modal.snapshotLive, "the snapshot must be live while shown")
        modal.shown = false
        verify(!modal.snapshotLive, "the snapshot must freeze on close")
        compare(modal.visible, true)
        tryCompare(modal, "visible", false)
    }

    // The author saw a ring light on a row of the usage section the moment
    // the dialog opened, and hop to the close button. Rows arriving after the
    // open must leave the focus where it was: on the layer, with no ring.
    function test_rows_arriving_after_the_open_do_not_move_the_focus() {
        openWithConsumerFocus()
        lateList.reset(function() {
            lateList.usageRows = [
                { id: 1, name: "one", kind: "folder", tone: "a", share: 0.6,
                  size: "6 MiB", percent: "60 %", detail: "" },
                { id: 2, name: "two", kind: "file", tone: "b", share: 0.4,
                  size: "4 MiB", percent: "40 %", detail: "" }]
        })
        wait(50)
        lateList.reset(function() {
            lateList.usageRows = [
                { id: 3, name: "three", kind: "folder", tone: "a", share: 1,
                  size: "1 MiB", percent: "100 %", detail: "" }]
        })
        wait(50)
        verify(modal.activeFocus, "the layer lost the focus to the rows")
        verify(!lastButton.activeFocus, "the focus hopped to the close button")
        verify(!lastButton.visualFocus, "the close button lit its ring")
    }

    function test_exit_fade_keeps_lower_surface_blocked() {
        CelestinaTheme.reducedMotion = false
        openWithConsumerFocus()
        tryCompare(modal, "opacity", 1)

        modal.shown = false
        compare(modal.visible, true)
        mouseClick(scene, 4, 4, Qt.LeftButton)
        compare(lowerClicks, 0)
        compare(dismissed, false)
        tryCompare(modal, "visible", false)
    }

    // The other side of the fade: the dialog's own controls are done the moment
    // it starts leaving. A primary button that answered a second click during
    // the 100 ms fade sent its request twice.
    function test_exit_fade_blocks_the_layer_content() {
        CelestinaTheme.reducedMotion = false
        openWithConsumerFocus()
        tryCompare(modal, "opacity", 1)

        mouseClick(lastButton, lastButton.width / 2,
                   lastButton.height / 2, Qt.LeftButton)
        compare(modalClicks, 1)

        modal.shown = false
        compare(modal.visible, true)
        verify(!lastButton.enabled, "the layer's content stays enabled while fading")
        mouseClick(lastButton, lastButton.width / 2,
                   lastButton.height / 2, Qt.LeftButton)
        mouseClick(lastButton, lastButton.width / 2,
                   lastButton.height / 2, Qt.LeftButton)
        compare(modalClicks, 1)
        // ...and those clicks did not reach the surface below either.
        compare(lowerClicks, 0)
        compare(dismissed, false)
        tryCompare(modal, "visible", false)
    }
}
