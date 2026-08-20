import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// Ctrl+V must reach the controller whenever a paste is conceivable — including
// when the clipboard was filled somewhere this controller has not been told
// about: another application, or another tab, whose internal clipboard is its
// own. `canPaste` answers the *menu*'s question, refreshed as the folder menu
// opens, and gating the shortcut on it left Ctrl+V silently dead.
TestCase {
    id: testCase
    name: "PasteShortcut"
    width: 420
    height: 320
    visible: true
    when: windowShown

    property int pastes: 0

    QtObject {
        id: controllerStub
        // False is the interesting value: it is what a controller reports until
        // the folder menu is opened.
        property bool canPaste: false
        property bool trashActive: false
        function paste() { testCase.pastes++ }
    }

    Item {
        id: host
        anchors.fill: parent
        property bool viewActive: true

        Shortcut {
            sequences: [StandardKey.Paste]
            enabled: host.viewActive && !controllerStub.trashActive
            onActivated: controllerStub.paste()
        }
    }

    function init() {
        testCase.pastes = 0
        controllerStub.canPaste = false
        controllerStub.trashActive = false
        host.viewActive = true
    }

    function test_a_paste_reaches_the_controller_with_a_stale_can_paste() {
        keyClick(Qt.Key_V, Qt.ControlModifier)
        compare(testCase.pastes, 1,
                "the shortcut is still gated on a stale canPaste")
    }

    function test_b_the_trash_still_refuses_a_paste() {
        controllerStub.trashActive = true
        keyClick(Qt.Key_V, Qt.ControlModifier)
        compare(testCase.pastes, 0)
    }

    function test_c_an_inactive_view_does_not_paste() {
        host.viewActive = false
        keyClick(Qt.Key_V, Qt.ControlModifier)
        compare(testCase.pastes, 0)
    }
}
