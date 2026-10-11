import QtQuick
import QtQuick.Controls
import org.celestina.selenita 1.0
import "components"

// Selenita's window: one window, no strip, the cards stacked and scrolled as
// one (design §5.3): the capture, the recording and the history; beside it
// the corner preview of the latest result and, while it records, the
// recording bar, each a window of its own.
// The window stays on screen while it captures (a capture may be of Selenita
// itself); a key-binding launch (`--screenshot`, `--record`,
// `--record-focused`) keeps it hidden and shows only the bar and the
// preview, unless the capture or the recording fails.
ApplicationWindow {
    id: window

    // The activation adapter, for the route below and for the tests.
    readonly property alias activation: activationAdapter
    // The corner preview, for the smoke and the tests.
    readonly property alias preview: previewWindow
    // The recording bar, for the smoke and the tests.
    readonly property alias recordingBar: recordingBarWindow

    // Takes a capture with the card's choices, as the button does.
    function startCapture() {
        captureCard.capture()
    }

    // Takes a key binding's capture: the card's choices, but a screen
    // capture takes the monitor niri has focused.
    function startBindingCapture() {
        captureCard.captureFromBinding()
    }

    width: 560
    height: 720
    minimumWidth: 420
    minimumHeight: 420
    // A key-binding launch with no Selenita running takes its capture or
    // recording without the window: the corner preview shows the result.
    visible: !SelenitaController.launchCapture
    // Closing the window ends Selenita, as it did before the preview: the
    // preview and the recording bar, top-level windows of their own, go at
    // once so Qt finds no window left and quits (a recording under way is
    // finished then).
    onClosing: {
        previewWindow.hideNow()
        recordingBarWindow.hideNow()
        if (SelenitaController.previewVisible)
            SelenitaController.dismissPreview()
    }
    // Transparent: the compositor blurs what lies behind the window and
    // CelestinaBackdrop lays the canvas over it (DESIGN §5.2 L0).
    color: CelestinaTheme.clear
    title: "Selenita"

    CelestinaBackdrop {
        anchors.fill: parent
    }

    // The suite's appearance file (reduced motion, text scale), followed by
    // the controller and bound into the theme once for this window.
    CelestinaAppearance {
        reducedMotion: SelenitaController.appearanceReducedMotion
        textScale: SelenitaController.appearanceTextScale
    }

    // Product copy the controller needs for the files it writes.
    Binding {
        target: SelenitaController
        property: "fileStem"
        value: qsTr("Captura")
    }

    // The captures folder as the card names it; the folder on disk is
    // `selenita_core::names::CAPTURES_FOLDER`, whatever this text says.
    Binding {
        target: SelenitaController
        property: "folderName"
        value: qsTr("Capturas")
    }

    Binding {
        target: SelenitaController
        property: "recordingStem"
        value: qsTr("Grabación")
    }

    // A second launch brings this window forward; a key binding's
    // `--screenshot` takes a capture.
    SelenitaActivation {
        id: activationAdapter
    }

    ActivationRoute {
        source: activationAdapter
        host: window
    }

    Connections {
        target: SelenitaController

        // A key-binding launch's capture failed: the notice needs the window.
        function onShowWindowRequested() {
            window.show()
            window.raise()
            window.requestActivate()
            page.reveal(historyCard)
        }

        // A key-binding launch that never showed the window ends with its
        // result: the preview gone, nothing capturing or recording.
        // The preview goes at once: Qt asks every window on screen to
        // close before it quits, and a fading preview would refuse.
        function onLaunchFinished() {
            if (!window.visible) {
                previewWindow.hideNow()
                recordingBarWindow.hideNow()
                Qt.quit()
            }
        }

        function onNotice(kind: string, text: string) {
            notice.show(kind, text)
        }

        function onRecordingFinished(entryId: string) {
            page.reveal(historyCard)
        }
    }

    // The keys of the window around the cards: a key no control took comes
    // up to the map, and the map holds the focus until a control takes it.
    KeyMap {
        id: keyMap
        objectName: "keyMap"
        anchors.fill: parent
        host: window

        PageScroll {
            id: page
            anchors.fill: parent

            CaptureCard {
                id: captureCard
                objectName: "captureCard"
                width: parent.width
            }

            RecordingCard {
                id: recordingCard
                objectName: "recordingCard"
                width: parent.width
            }

            HistoryCard {
                id: historyCard
                objectName: "historyCard"
                width: parent.width
            }
        }
    }

    // Whether the pointer gave an item its focus: a row says so itself, a
    // control through its reason. Untyped, as the item may be either.
    function pointerGave(item: var): bool {
        return item.pointerFocused === true || item.focusReason === Qt.MouseFocusReason
    }

    // A control reached with the keyboard is brought into view; one clicked
    // is where the pointer is.
    onActiveFocusItemChanged: {
        const item = window.activeFocusItem
        if (item && !window.pointerGave(item))
            page.reveal(item)
    }

    // The corner preview: its own frameless window, placed by niri's rule.
    PreviewWindow {
        id: previewWindow
    }

    // The recording bar: its own frameless window, placed by niri's rule.
    RecordingBar {
        id: recordingBarWindow
    }

    NoticePill {
        id: notice
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.windowMargin
    }

    // Development only: the smoke takes one capture over the fakes, starts,
    // pauses, resumes and stops one fake recording, and reads what the
    // window shows after.
    Timer {
        id: smokeCapture
        interval: 1000
        running: SelenitaController.smokeReport && SelenitaController.fake
                 && !SelenitaController.launchCapture
        onTriggered: window.startCapture()
    }

    Timer {
        interval: 1500
        running: SelenitaController.smokeReport && SelenitaController.fake
        onTriggered: SelenitaController.toggleRecording()
    }

    Timer {
        interval: 2500
        running: SelenitaController.smokeReport && SelenitaController.fake
        onTriggered: SelenitaController.stopRecording()
    }

    // Development only: the smoke reads the recording bar while the fake
    // recording runs (at 1.65 s) and while it is paused (at 1.95 s): the
    // recording pauses at 1.75 s and resumes at 2.15 s, so its stop joins
    // two segments (SEL-4-B).
    function logSmokeBar() {
        console.log("selenita-smoke-bar:"
                    + " recording=" + SelenitaController.recordingState
                    + " bar=" + (recordingBarWindow.visible ? "shown" : "hidden")
                    + " title=" + recordingBarWindow.title
                    + " elapsed=" + recordingBarWindow.elapsed)
    }

    Timer {
        interval: 1650
        running: SelenitaController.smokeReport && SelenitaController.fake
        onTriggered: window.logSmokeBar()
    }

    Timer {
        interval: 1750
        running: SelenitaController.smokeReport && SelenitaController.fake
        onTriggered: SelenitaController.pauseRecording()
    }

    Timer {
        interval: 1950
        running: SelenitaController.smokeReport && SelenitaController.fake
        onTriggered: window.logSmokeBar()
    }

    Timer {
        interval: 2150
        running: SelenitaController.smokeReport && SelenitaController.fake
        onTriggered: SelenitaController.resumeRecording()
    }

    // Development only: a `--record-focused` launch's smoke reads the bar
    // while the fake recording runs, discards the recording from it as its
    // «Descartar» does and expects the process to end with no file.
    Timer {
        interval: 1500
        running: SelenitaController.smokeDiscard && SelenitaController.fake
        onTriggered: {
            console.log("selenita-smoke-discard:"
                        + " recording=" + SelenitaController.recordingState
                        + " bar=" + (recordingBarWindow.visible ? "shown" : "hidden")
                        + " shown=" + window.visible
                        + " preview=" + (previewWindow.visible ? "shown" : "hidden"))
            SelenitaController.discardRecording()
        }
    }

    // Development only: a key-binding launch's smoke dismisses the preview
    // after the report, as its × does, and expects the process to end (the
    // offscreen pointer rests at the preview's corner and holds its timer).
    Timer {
        interval: 5000
        running: SelenitaController.smokeReport && SelenitaController.fake
                 && SelenitaController.launchCapture
        onTriggered: SelenitaController.dismissPreview()
    }

    // Development only: the smoke closes the window while the preview
    // shows and a fake recording runs, and expects the process to end.
    Timer {
        interval: 2000
        running: SelenitaController.smokeClose && SelenitaController.fake
        onTriggered: window.close()
    }

    Timer {
        interval: 4000
        running: SelenitaController.smokeReport
        onTriggered: console.log("selenita-smoke:"
                                 + " cards=" + [captureCard, recordingCard, historyCard]
                                       .filter(card => card.visible).length
                                 + " history=" + historyCard.count
                                 + " recording=" + SelenitaController.recordingState
                                 + " lastRecording=" + (SelenitaController.lastRecordingName !== "")
                                 + " shown=" + window.visible
                                 + " preview=" + (previewWindow.visible ? "shown" : "hidden")
                                 + " bar=" + (recordingBarWindow.visible ? "shown" : "hidden")
                                 + " fake=" + SelenitaController.fake
                                 + " textScale=" + CelestinaTheme.textScale
                                 + " fontBody=" + CelestinaTheme.fontBody)
    }

    Component.onCompleted: {
        if (SelenitaController.smokeReport)
            console.log("selenita-smoke-start: shown=" + window.visible)
        activationAdapter.start()
    }
}
