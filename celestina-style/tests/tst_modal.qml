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
        tryCompare(firstField, "activeFocus", true)
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

    function test_enters_first_focusable_automatically() {
        openWithConsumerFocus()
        verify(firstField.activeFocus)
        verify(!lowerButton.activeFocus)
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
        tryCompare(middleCheck, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(middleList, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(lastButton, "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(firstField, "activeFocus", true)

        keyClick(Qt.Key_Backtab)
        tryCompare(lastButton, "activeFocus", true)
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
