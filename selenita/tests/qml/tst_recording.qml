import QtQuick
import QtTest 1.3
import org.celestina.selenita 1.0

// The recording card over the stub controller (a fake portal and recorder
// that answer at once): the button starts and stops, the dot and the time
// show while it records, the sound switch, the missing-muxer line and the
// last recording's «Abrir en Fluorita».
TestCase {
    id: testCase
    name: "RecordingCard"
    visible: true
    when: windowShown
    width: 560
    height: 720

    Component {
        id: cardComponent
        RecordingCard { width: 520 }
    }

    function init() {
        SelenitaController.reset()
        SelenitaController.recordingStem = "Recording"
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

    function test_the_button_starts_and_stops_a_recording() {
        const card = createTemporaryObject(cardComponent, testCase)
        const button = named(card, "recordButton")
        const dot = named(card, "recordingDot")
        compare(button.text, qsTr("Grabar"))
        verify(!dot.visible)
        mouseClick(button)
        compare(SelenitaController.recordingState, "recording")
        compare(button.text, qsTr("Detener"))
        verify(dot.visible)
        verify(named(card, "elapsedText").text.match(/^\d+:\d\d$/) !== null)
        mouseClick(button)
        compare(SelenitaController.recordingState, "idle")
        compare(SelenitaController.calls, ["toggleRecording:false", "toggleRecording:false"])
        verify(!dot.visible)
        compare(button.text, qsTr("Grabar"))
    }

    function test_the_elapsed_time_reads_from_the_start() {
        const card = createTemporaryObject(cardComponent, testCase)
        compare(card.elapsedText(5), "0:05")
        compare(card.elapsedText(65), "1:05")
        compare(card.elapsedText(3725), "1:02:05")
        SelenitaController.recordingStartedAt = Date.now() - 61500
        SelenitaController.recordingState = "recording"
        compare(named(card, "elapsedText").text, "1:01")
    }

    function test_the_sound_switch_is_chosen_before_recording() {
        const card = createTemporaryObject(cardComponent, testCase)
        const toggle = named(named(card, "audioRow"), "settingSwitch")
        verify(!toggle.checked)
        mouseClick(toggle)
        verify(SelenitaController.withAudio)
        verify(toggle.checked)
        mouseClick(named(card, "recordButton"))
        verify(!toggle.enabled, "no change while it records")
        compare(SelenitaController.calls[0], "toggleRecording:true")
    }

    function test_the_transit_states_disable_the_button() {
        const card = createTemporaryObject(cardComponent, testCase)
        SelenitaController.recordingState = "preparing"
        verify(!named(card, "recordButton").enabled)
        verify(named(card, "elapsedText").text.length > 0)
        SelenitaController.recordingState = "stopping"
        verify(!named(card, "recordButton").enabled)
        SelenitaController.recordingState = "idle"
        verify(named(card, "recordButton").enabled)
    }

    function test_a_missing_muxer_is_said_and_disables_the_button() {
        const card = createTemporaryObject(cardComponent, testCase)
        SelenitaController.recorderMissing = "mp4mux"
        verify(!named(card, "recordButton").enabled)
        verify(named(card, "elapsedText").text.indexOf("mp4mux") >= 0)
        SelenitaController.recorderMissing = ""
        verify(named(card, "recordButton").enabled)
    }

    function test_the_last_recording_opens_in_fluorita() {
        const card = createTemporaryObject(cardComponent, testCase)
        const row = named(card, "lastRecordingRow")
        verify(!row.visible)
        mouseClick(named(card, "recordButton"))
        mouseClick(named(card, "recordButton"))
        verify(row.visible)
        compare(named(card, "lastRecordingName").text, "Recording 1.mp4")
        // The row's place in the card settles on the next frame.
        waitForRendering(card)
        mouseClick(named(card, "openLastRecording"))
        compare(SelenitaController.calls[2], "openInFluorita:/fake/Recording 1.mp4")
        compare(SelenitaController.history[0].kind, "recording")
    }
}
