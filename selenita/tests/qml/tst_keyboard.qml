import QtQuick
import QtTest 1.3
import org.celestina.selenita 1.0

// The keyboard and accessibility pass over the whole window (design §5.3):
// 1, 2 and 3 choose the target, Enter captures, R toggles the recording, Tab
// walks the controls with a visible ring and Escape brings the focus back to
// the window; a history row is a Tab stop the arrows walk, Enter opens and
// Delete trashes; none of it fires while a text field has the focus; the
// rows and the controls read their names to a screen reader.
TestCase {
    id: testCase
    name: "Keyboard"
    when: windowShown

    Component {
        id: mainComponent
        Main {}
    }

    // The window under test, closed before the next test opens its own so
    // only one holds the keyboard.
    property var current: null

    function init() {
        SelenitaController.reset()
    }

    function cleanup() {
        if (testCase.current !== null)
            testCase.current.close()
        testCase.current = null
        wait(0)
    }

    function findNamed(node, name) {
        if (node.objectName === name)
            return node
        const kids = node.children !== undefined ? node.children : []
        let found = null
        let i = 0
        while (found === null && i < kids.length) {
            found = findNamed(kids[i], name)
            i += 1
        }
        return found
    }

    function named(root, name) {
        const item = findNamed(root, name)
        verify(item !== null, name)
        return item
    }

    function openWindow() {
        const window = createTemporaryObject(mainComponent, testCase)
        testCase.current = window
        tryVerify(() => window.visible)
        window.requestActivate()
        tryVerify(() => window.active, 5000, "the window takes the keyboard")
        const keyMap = named(window.contentItem, "keyMap")
        tryVerify(() => keyMap.activeFocus, 5000, "the key map holds the focus at start")
        return window
    }

    function takeTwo() {
        SelenitaController.capture(true, "#3e91ff", "#40000000")
        SelenitaController.capture(true, "#3e91ff", "#40000000")
        compare(SelenitaController.history.length, 2)
    }

    function test_the_digits_choose_the_target() {
        const window = openWindow()
        keyClick(Qt.Key_2)
        compare(SelenitaController.target, "window")
        verify(named(window.contentItem, "targetWindow").checked)
        keyClick(Qt.Key_3)
        compare(SelenitaController.target, "region")
        keyClick(Qt.Key_1)
        compare(SelenitaController.target, "screen")
        // A modified digit is somebody else's key.
        keyClick(Qt.Key_2, Qt.ControlModifier)
        compare(SelenitaController.target, "screen")
        compare(SelenitaController.calls, [])
    }

    function test_enter_takes_the_capture_with_the_cards_choices() {
        const window = openWindow()
        keyClick(Qt.Key_3)
        keyClick(Qt.Key_Return)
        compare(SelenitaController.calls.length, 1)
        verify(SelenitaController.calls[0].startsWith("capture:region,0,true,true,true,"))
        compare(SelenitaController.history.length, 1)
        // From a focused switch the key still reaches the window's map.
        const toggle = named(named(window.contentItem, "clipboardRow"), "settingSwitch")
        toggle.forceActiveFocus(Qt.TabFocusReason)
        tryVerify(() => toggle.activeFocus)
        keyClick(Qt.Key_Enter)
        compare(SelenitaController.calls.length, 2)
        compare(SelenitaController.history.length, 2)
    }

    function test_enter_on_a_focused_button_presses_it_and_does_not_capture() {
        const window = openWindow()
        const content = window.contentItem
        // A target button chooses its target.
        SelenitaController.target = "region"
        const screen = named(content, "targetScreen")
        screen.forceActiveFocus(Qt.TabFocusReason)
        tryVerify(() => screen.activeFocus)
        keyClick(Qt.Key_Return)
        compare(SelenitaController.target, "screen")
        compare(SelenitaController.calls, [])
        // The last recording's button opens it.
        SelenitaController.toggleRecording()
        SelenitaController.stopRecording()
        SelenitaController.calls = []
        const open = named(content, "openLastRecording")
        tryVerify(() => open.visible)
        open.forceActiveFocus(Qt.TabFocusReason)
        tryVerify(() => open.activeFocus)
        keyClick(Qt.Key_Enter)
        compare(SelenitaController.calls, ["openInFluorita:" + SelenitaController.lastRecordingId])
        // A row's trash icon trashes.
        const row = named(named(content, "historyCard"), "historyRow0")
        const id = row.entry.id
        const trash = named(row, "deleteAction")
        trash.forceActiveFocus(Qt.TabFocusReason)
        tryVerify(() => trash.activeFocus)
        keyClick(Qt.Key_Return)
        compare(SelenitaController.calls, ["openInFluorita:" + id, "deleteEntry:" + id])
        compare(SelenitaController.history.length, 0)
    }

    function test_r_toggles_the_recording() {
        openWindow()
        keyClick(Qt.Key_R)
        compare(SelenitaController.recordingState, "recording")
        keyClick(Qt.Key_R)
        compare(SelenitaController.recordingState, "idle")
        compare(SelenitaController.calls, ["toggleRecording:false", "toggleRecording:false"])
        compare(SelenitaController.history.length, 1)
        compare(SelenitaController.history[0].kind, "recording")
    }

    function test_the_keys_stay_quiet_in_a_text_field() {
        const window = openWindow()
        const field = Qt.createQmlObject(
            "import QtQuick; TextInput { objectName: 'probeField'; width: 100; height: 20 }",
            window.contentItem, "probeField")
        field.forceActiveFocus()
        tryVerify(() => field.activeFocus)
        keyClick(Qt.Key_2)
        keyClick(Qt.Key_R)
        keyClick(Qt.Key_Return)
        keyClick(Qt.Key_1)
        compare(field.text, "2r1")
        compare(SelenitaController.target, "screen")
        compare(SelenitaController.recordingState, "idle")
        compare(SelenitaController.calls, [])
        field.destroy()
    }

    function test_tab_lights_a_control_and_escape_returns_to_the_window() {
        const window = openWindow()
        const keyMap = named(window.contentItem, "keyMap")
        const first = named(window.contentItem, "targetScreen")
        keyClick(Qt.Key_Tab)
        tryVerify(() => first.activeFocus, 5000, "the first Tab reaches the first target")
        verify(first.visualFocus, "a Tab shows the ring")
        keyClick(Qt.Key_Tab)
        verify(named(window.contentItem, "targetWindow").activeFocus)
        keyClick(Qt.Key_Tab, Qt.ShiftModifier)
        verify(first.activeFocus)
        keyClick(Qt.Key_Escape)
        tryVerify(() => keyMap.activeFocus, 5000, "Escape brings the focus back")
        verify(!first.activeFocus)
        compare(SelenitaController.calls, [])
    }

    function test_the_arrows_walk_the_history_and_enter_opens_a_row() {
        const window = openWindow()
        takeTwo()
        const history = named(window.contentItem, "historyCard")
        const first = named(history, "historyRow0")
        const second = named(history, "historyRow1")
        verify(first.activeFocusOnTab, "a row is a Tab stop")
        first.forceActiveFocus(Qt.TabFocusReason)
        tryVerify(() => first.activeFocus)
        verify(first.visualFocus, "a keyboard focus wears the ring")
        keyClick(Qt.Key_Down)
        tryVerify(() => second.activeFocus)
        keyClick(Qt.Key_Down)
        verify(second.activeFocus, "the last row is the end")
        keyClick(Qt.Key_Up)
        tryVerify(() => first.activeFocus)
        keyClick(Qt.Key_End)
        tryVerify(() => second.activeFocus)
        keyClick(Qt.Key_Home)
        tryVerify(() => first.activeFocus)
        SelenitaController.calls = []
        keyClick(Qt.Key_Return)
        compare(SelenitaController.calls, ["openInFluorita:" + first.entry.id])
        // A pointer click selects a row without the ring. The rows take their
        // places in the card on the next frame.
        tryVerify(() => second.y > first.y, 5000, "the rows are laid out")
        mouseClick(second, second.width / 2, second.height / 2)
        tryVerify(() => second.activeFocus)
        verify(!second.visualFocus)
    }

    function test_delete_trashes_the_focused_row_and_the_next_takes_the_focus() {
        const window = openWindow()
        takeTwo()
        const history = named(window.contentItem, "historyCard")
        const first = named(history, "historyRow0")
        const doomed = first.entry.id
        first.forceActiveFocus(Qt.TabFocusReason)
        tryVerify(() => first.activeFocus)
        SelenitaController.calls = []
        keyClick(Qt.Key_Delete)
        compare(SelenitaController.calls, ["deleteEntry:" + doomed])
        compare(history.count, 1)
        const survivor = named(history, "historyRow0")
        verify(survivor.entry.id !== doomed)
        tryVerify(() => survivor.activeFocus, 5000, "the row that takes its place is focused")
        verify(survivor.visualFocus)
        // From an action button inside the row the key still reaches the row.
        named(survivor, "openAction").forceActiveFocus(Qt.TabFocusReason)
        keyClick(Qt.Key_Delete)
        compare(SelenitaController.calls.length, 2)
        compare(history.count, 0)
        verify(named(history, "historyEmpty").visible)
    }

    function test_the_rows_and_the_controls_read_their_names() {
        const window = openWindow()
        takeTwo()
        SelenitaController.toggleRecording()
        SelenitaController.stopRecording()
        const content = window.contentItem
        const history = named(content, "historyCard")
        const recording = named(history, "historyRow0")
        compare(recording.Accessible.role, Accessible.ListItem)
        compare(recording.Accessible.name,
                [qsTr("Grabación") + " 3.mp4", qsTr("Grabación"), qsTr("%1 B").arg(17),
                 recording.takenText].join(", "))
        const still = named(history, "historyRow1")
        verify(still.Accessible.name.startsWith(qsTr("Captura") + " 2.png, " + qsTr("Captura")
                                                + ", " + qsTr("%1 B").arg(70) + ", "))
        compare(named(content, "lastRecordingName").Accessible.name,
                qsTr("Última grabación: %1").arg(qsTr("Grabación") + " 3.mp4"))
        compare(named(content, "targetRegion").Accessible.description, qsTr("Tecla %1").arg(3))
        compare(named(content, "captureButton").Accessible.description, qsTr("Intro"))
        compare(named(content, "recordButton").Accessible.description, qsTr("Tecla R"))
        const clipboard = named(named(content, "clipboardRow"), "settingSwitch")
        compare(clipboard.Accessible.role, Accessible.CheckBox)
        compare(clipboard.Accessible.name, qsTr("Copiar al portapapeles"))
        SelenitaController.target = "window"
        compare(clipboard.Accessible.description, qsTr("niri copia la ventana al portapapeles"))
        compare(named(content, "captureCard").Accessible.name, qsTr("Captura"))
        verify(named(named(content, "captureCard"), "sectionLabel").Accessible.ignored,
               "the eyebrow is not read twice")
    }

    function test_an_owed_focus_is_dropped_when_the_trash_fails_or_the_focus_leaves() {
        const window = openWindow()
        takeTwo()
        const history = named(window.contentItem, "historyCard")
        const first = named(history, "historyRow0")
        first.forceActiveFocus(Qt.TabFocusReason)
        tryVerify(() => first.activeFocus)
        history.owedIndex = 0
        SelenitaController.notice("error", "the trash refused")
        compare(history.owedIndex, -1)
        history.owedIndex = 0
        named(window.contentItem, "targetScreen").forceActiveFocus(Qt.TabFocusReason)
        tryCompare(history, "owedIndex", -1)
        // A second Delete while one is in flight is ignored.
        first.forceActiveFocus(Qt.TabFocusReason)
        tryVerify(() => first.activeFocus)
        history.owedIndex = 1
        SelenitaController.calls = []
        keyClick(Qt.Key_Delete)
        compare(SelenitaController.calls, [])
        compare(history.count, 2)
    }
}
