import QtQuick
import QtQuick.Controls
import org.celestina.selenita 1.0
import "components"

// Selenita's window: one window, no strip, the cards stacked and scrolled as
// one (design §5.3): the capture, the recording and the history.
// The window steps aside while a capture would include it and comes back on
// the history once it is taken.
ApplicationWindow {
    id: window

    // The activation adapter, for the route below and for the tests.
    readonly property alias activation: activationAdapter

    // Takes a capture with the card's choices, as the button does.
    function startCapture() {
        captureCard.capture()
    }

    width: 560
    height: 720
    minimumWidth: 420
    minimumHeight: 420
    // A `--screenshot` launch with no Selenita running takes its capture
    // before the window ever shows, then shows it on the history.
    visible: !SelenitaController.launchCapture
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

        function onHideWindowRequested() {
            window.hide()
        }

        function onShowWindowRequested() {
            window.show()
            window.raise()
            window.requestActivate()
            page.reveal(historyCard)
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
                windowShown: window.visible
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
