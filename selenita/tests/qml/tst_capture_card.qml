import QtQuick
import QtTest 1.3
import org.celestina.selenita 1.0

// The capture card over the stub controller: the three targets, the delay,
// the two destinations, and a capture that lands in the history.
TestCase {
    id: testCase
    name: "CaptureCard"
    visible: true
    when: windowShown
    width: 560
    height: 720

    Component {
        id: cardComponent
        CaptureCard { width: 520 }
    }

    Component {
        id: historyComponent
        HistoryCard { width: 520 }
    }

    function init() {
        SelenitaController.reset()
        SelenitaController.fileStem = "Captura"
        SelenitaController.folderName = "Capturas"
    }

    function findNamed(node, name) {
        if (node.objectName === name)
            return node
        const kids = node.children !== undefined ? node.children : []
        for (let i = 0; i < kids.length; i += 1) {
            const found = findNamed(kids[i], name)
            if (found !== null)
                return found
        }
        return null
    }

    function named(card, name) {
        const item = findNamed(card, name)
        verify(item !== null, name)
        return item
    }

    function test_the_three_targets_set_the_target() {
        const card = createTemporaryObject(cardComponent, testCase)
        const buttons = { targetWindow: "window", targetRegion: "region", targetScreen: "screen" }
        for (const name in buttons) {
            const button = named(card, name)
            mouseClick(button)
            compare(SelenitaController.target, buttons[name], name)
            verify(button.checked, name + " shows its choice")
        }
        verify(!named(card, "targetWindow").checked)
    }

    function test_the_delay_choice() {
        const card = createTemporaryObject(cardComponent, testCase)
        mouseClick(named(card, "delay5"))
        compare(SelenitaController.delay, 5)
        verify(named(card, "delay5").checked)
        verify(!named(card, "delay0").checked)
        mouseClick(named(card, "delay10"))
        compare(SelenitaController.delay, 10)
        mouseClick(named(card, "delay0"))
        compare(SelenitaController.delay, 0)
    }

    function test_the_destination_switches() {
        const card = createTemporaryObject(cardComponent, testCase)
        const clipboard = named(named(card, "clipboardRow"), "settingSwitch")
        const file = named(named(card, "fileRow"), "settingSwitch")
        verify(clipboard.checked && file.checked)
        mouseClick(clipboard)
        verify(!SelenitaController.toClipboard)
        mouseClick(file)
        verify(!SelenitaController.toFile)
        verify(!named(card, "captureButton").enabled, "no destination, no capture")
        mouseClick(clipboard)
        verify(SelenitaController.toClipboard)
        verify(named(card, "captureButton").enabled)
    }

    function test_a_window_capture_says_niri_copies_it() {
        const card = createTemporaryObject(cardComponent, testCase)
        const row = named(card, "clipboardRow")
        const toggle = named(row, "settingSwitch")
        SelenitaController.toClipboard = false
        SelenitaController.toFile = false
        verify(!named(row, "settingHint").visible)
        mouseClick(named(card, "targetWindow"))
        verify(!toggle.enabled)
        verify(toggle.checked)
        verify(named(row, "settingHint").visible)
        verify(named(card, "captureButton").enabled, "niri's copy is a destination")
        mouseClick(named(card, "targetScreen"))
        verify(toggle.enabled)
        verify(!toggle.checked)
    }

    function test_the_output_choice_shows_only_with_several_outputs() {
        const card = createTemporaryObject(cardComponent, testCase)
        verify(!named(card, "outputRow").visible)
        SelenitaController.outputs = ["HDMI-A-1", "DP-1"]
        verify(named(card, "outputRow").visible)
        SelenitaController.target = "region"
        verify(!named(card, "outputRow").visible)
    }

    function test_capturing_over_the_fake_yields_a_history_row_with_a_thumbnail() {
        const card = createTemporaryObject(cardComponent, testCase)
        const history = createTemporaryObject(historyComponent, testCase, { y: 400 })
        verify(named(history, "historyEmpty").visible)
        mouseClick(named(card, "targetRegion"))
        mouseClick(named(card, "delay3"))
        mouseClick(named(card, "captureButton"))
        compare(SelenitaController.calls.length, 1)
        verify(SelenitaController.calls[0].startsWith("capture:region,3,true,true,true,"))
        compare(history.count, 1)
        verify(!named(history, "historyEmpty").visible)
        const thumbnail = named(named(history, "historyRow0"), "thumbnail")
        tryCompare(thumbnail, "status", Image.Ready)
        verify(thumbnail.source.toString().endsWith("pixel.png"))
    }

    function test_the_countdown_shows_the_seconds_left() {
        const card = createTemporaryObject(cardComponent, testCase)
        const text = named(card, "countdownText")
        verify(!text.visible)
        SelenitaController.countdown(3)
        verify(text.visible)
        verify(text.text.indexOf("3") >= 0)
        SelenitaController.countdown(0)
        verify(!text.visible)
    }
}
