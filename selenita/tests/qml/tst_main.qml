import QtQuick
import QtTest 1.3
import org.celestina.selenita 1.0

// The whole window, over QML stand-ins for the Rust types: it constructs,
// starts its activation adapter, shows the three cards in order, a second
// launch brings it forward, a key binding's capture is taken, the window
// stays for a capture, a key-binding launch shows only the corner preview
// (the window only when it fails), and a file handed back goes to the
// controller.
TestCase {
    id: testCase
    name: "Main"
    when: windowShown

    Component {
        id: mainComponent
        Main {}
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

    function test_the_window_constructs_with_three_cards() {
        const window = createTemporaryObject(mainComponent, testCase)
        verify(window)
        tryVerify(function() { return window.visible })
        const names = ["captureCard", "recordingCard", "historyCard"]
        const titles = [qsTr("Captura"), qsTr("Grabación"), qsTr("Historial")]
        let previousBottom = -1
        for (let i = 0; i < names.length; i += 1) {
            const card = findNamed(window.contentItem, names[i])
            verify(card, names[i])
            verify(card.visible, names[i] + " is shown")
            compare(findNamed(card, "sectionLabel").text, titles[i].toUpperCase())
            verify(card.y > previousBottom, names[i] + " stacks below the one before")
            previousBottom = card.y + card.height
        }
    }

    function test_the_activation_adapter_starts_once() {
        const window = createTemporaryObject(mainComponent, testCase)
        compare(window.activation.starts, 1)
    }

    function test_a_key_binding_capture_chooses_the_target_and_captures() {
        SelenitaController.reset()
        const window = createTemporaryObject(mainComponent, testCase)
        tryVerify(function() { return window.visible })
        window.activation.captureRequested("window")
        compare(SelenitaController.target, "window")
        compare(SelenitaController.calls.length, 1)
        verify(SelenitaController.calls[0].startsWith("capture:window,"))
        compare(SelenitaController.fileStem, qsTr("Captura"))
        compare(SelenitaController.folderName, qsTr("Capturas"))
    }

    function test_a_key_binding_toggles_and_stops_the_recording() {
        SelenitaController.reset()
        const window = createTemporaryObject(mainComponent, testCase)
        tryVerify(function() { return window.visible })
        compare(SelenitaController.recordingStem, qsTr("Grabación"))
        window.activation.recordingRequested("toggle")
        compare(SelenitaController.recordingState, "recording")
        window.activation.recordingRequested("stop")
        compare(SelenitaController.recordingState, "idle")
        compare(SelenitaController.calls, ["toggleRecording:false", "stopRecording:recording"])
        compare(SelenitaController.history.length, 1)
        compare(SelenitaController.history[0].kind, "recording")
    }

    // A capture asked from the window never hides it (the person may be
    // capturing Selenita); its preview shows beside it.
    function test_the_window_stays_for_a_capture_and_the_preview_shows() {
        SelenitaController.reset()
        SelenitaController.previewOnResult = true
        const window = createTemporaryObject(mainComponent, testCase)
        tryVerify(function() { return window.visible })
        window.startCapture()
        verify(window.visible)
        compare(SelenitaController.history.length, 1)
        verify(window.preview.visible)
    }

    // A key-binding launch (`--screenshot`, `--record`) never shows the
    // main window when its capture ends: only the preview shows. A failed
    // capture shows the window, so its notice can be read.
    function test_a_key_binding_launch_shows_only_the_preview() {
        SelenitaController.reset()
        SelenitaController.launchCapture = true
        SelenitaController.previewOnResult = true
        const window = createTemporaryObject(mainComponent, testCase)
        verify(!window.visible)
        window.activation.captureRequested("screen")
        compare(SelenitaController.history.length, 1)
        const preview = window.preview
        verify(preview.visible, "the preview shows the capture")
        wait(100)
        verify(!window.visible, "the main window stays hidden")
        window.activation.recordingRequested("toggle")
        window.activation.recordingRequested("stop")
        verify(preview.visible)
        compare(SelenitaController.previewKind, "recording")
        verify(!window.visible)
        SelenitaController.showWindowRequested()
        verify(window.visible, "a failure shows the window")
        SelenitaController.launchCapture = false
    }

    // Closing the window with the preview on screen hides the preview at
    // once (Qt then finds no window left and quits Selenita, which
    // finishes a recording under way) and tells the controller.
    function test_closing_the_window_takes_the_preview_with_it() {
        SelenitaController.reset()
        SelenitaController.previewOnResult = true
        const window = createTemporaryObject(mainComponent, testCase)
        tryVerify(function() { return window.visible })
        window.startCapture()
        verify(window.preview.visible)
        verify(SelenitaController.previewVisible)
        window.close()
        verify(!window.visible)
        verify(!window.preview.visible, "the preview went with the window, at once")
        verify(!SelenitaController.previewVisible)
        verify(SelenitaController.calls.indexOf("dismissPreview:/fake/Captura 1.png") >= 0)
    }

    function test_a_file_handed_back_goes_to_the_controller() {
        SelenitaController.reset()
        const window = createTemporaryObject(mainComponent, testCase)
        window.activation.adoptRequested("/pictures/Capturas/Captura%201.png")
        compare(SelenitaController.calls, ["adopt:/pictures/Capturas/Captura%201.png"])
    }

    function test_a_second_launch_raises_the_window() {
        const window = createTemporaryObject(mainComponent, testCase)
        tryVerify(function() { return window.visible })
        window.hide()
        verify(!window.visible)
        window.activation.raiseRequested()
        verify(window.visible)
    }
}
