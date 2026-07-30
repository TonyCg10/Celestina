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

            MouseArea {
                anchors.fill: parent
                onClicked: testCase.lowerClicks += 1
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
}
