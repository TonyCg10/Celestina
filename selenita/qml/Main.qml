import QtQuick
import QtQuick.Controls
import org.celestina.selenita 1.0
import "components"

// Selenita's window: one window, no strip, the cards stacked and scrolled as
// one (design §5.3): the capture, the recording and the history; beside it
// the corner preview of the latest result, a window of its own.
// The window stays on screen while it captures (a capture may be of Selenita
// itself); a key-binding launch (`--screenshot`, `--record`) keeps it hidden
// and shows only the preview, unless the capture fails.
ApplicationWindow {
    id: window

    // The activation adapter, for the route below and for the tests.
    readonly property alias activation: activationAdapter
    // The corner preview, for the smoke and the tests.
    readonly property alias preview: previewWindow

    // Takes a capture with the card's choices, as the button does.
    function startCapture() {
        captureCard.capture()
    }

    width: 560
    height: 720
    minimumWidth: 420
    minimumHeight: 420
    // A key-binding launch with no Selenita running takes its capture or
    // recording without the window: the corner preview shows the result.
    visible: !SelenitaController.launchCapture
    // Closing the window ends Selenita, as it did before the preview: the
    // preview, a top-level window of its own, goes at once so Qt finds no
    // window left and quits (a recording under way is finished then).
    onClosing: {
        previewWindow.hideNow()
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

    NoticePill {
        id: notice
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.windowMargin
    }

    // Development only: the smoke takes one capture over the fakes, starts
    // and stops one fake recording, and reads what the window shows after.
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
