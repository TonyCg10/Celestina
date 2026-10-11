import QtQuick
import org.celestina.selenita 1.0
import "components"

// The recording bar (SEL-4-A): while a recording is under way, however it
// started (the window, `--record`, `--record-focused`, the bus), a small
// glass pill shows the red dot, the elapsed time and two actions:
// «Detener» stops and saves (the corner preview follows, as from the card)
// and «Descartar» stops and deletes the file (no history row, no preview).
// It is its own frameless top-level window, titled «Grabando», which niri's
// rule (matching Selenita's app id and that title) places at the top centre
// of the focused output without the focus; after a key-binding launch it is
// the only window on screen while it records. It never takes the keyboard
// focus: the card's «Detener» and `R` are the keyboard's way. A
// compositor's close is refused while it shows (closing is not a stop);
// once hidden, the application's quit closes it. The state is the
// controller's; the bar only shows it and asks. The actions are one row, so
// the next unit's pause button joins them and the pill grows to fit.
Window {
    id: bar
    objectName: "recordingBar"

    readonly property bool recording: SelenitaController.recordingState === "recording"
    // Set once the main window closes: the bar goes at once so Qt finds no
    // window left and quits (the recording is finished on the way out).
    property bool closingDown: false
    // For the tests: the clock and whether the dot pulses.
    readonly property alias elapsed: clock.elapsed
    readonly property alias pulsing: pulse.running

    // Hides at once, for good: the application is ending.
    function hideNow() {
        bar.closingDown = true
    }

    title: qsTr("Grabando")
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowDoesNotAcceptFocus
    // Its own top-level window: not a dialog of the main window, which may
    // be hidden (a key-binding launch never shows it).
    transientParent: null
    visible: bar.recording && !bar.closingDown
    width: pill.implicitWidth
    height: pill.implicitHeight
    // Transparent: the compositor blurs what lies behind it and the pill
    // lays the glass tint over that, as the windows' backdrop does.
    color: CelestinaTheme.clear

    onClosing: (close) => {
        if (bar.visible)
            close.accepted = false
    }

    RecordingClock {
        id: clock
        active: bar.recording
    }

    // The widest time the bar shows, so the pill does not change width as
    // the seconds go by.
    TextMetrics {
        id: widest
        font: elapsedText.font
        text: "00:00:00"
    }

    Rectangle {
        id: pill
        objectName: "recordingBarPill"
        anchors.fill: parent
        // Whole pixels: the window's size is in whole pixels.
        implicitWidth: Math.ceil(content.implicitWidth) + CelestinaTheme.spaceMd * 2
        implicitHeight: Math.ceil(content.implicitHeight) + CelestinaTheme.spaceSm * 2
        radius: CelestinaTheme.radiusPill
        color: CelestinaTheme.glassTint
        Accessible.role: Accessible.ToolBar
        Accessible.name: bar.title

        Row {
            id: content
            anchors.centerIn: parent
            spacing: CelestinaTheme.spaceSm

            Rectangle {
                id: dot
                objectName: "recordingBarDot"
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.spaceMd
                height: width
                radius: width / 2
                color: CelestinaTheme.danger
                Accessible.ignored: true

                // A slow pulse while it records; still under reduced motion.
                SequentialAnimation on opacity {
                    id: pulse
                    running: bar.recording && !CelestinaTheme.reducedMotion
                    loops: Animation.Infinite
                    alwaysRunToEnd: true
                    NumberAnimation { to: 0.35; duration: CelestinaTheme.motionSlow }
                    NumberAnimation { to: 1; duration: CelestinaTheme.motionSlow }
                }
            }

            Text {
                id: elapsedText
                objectName: "recordingBarElapsed"
                anchors.verticalCenter: parent.verticalCenter
                width: Math.ceil(widest.advanceWidth)
                horizontalAlignment: Text.AlignHCenter
                text: clock.text
                color: CelestinaTheme.text
                font.family: CelestinaTheme.monoFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.features: { "tnum": 1 }
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Tiempo grabado: %1").arg(clock.text)
            }

            Row {
                id: actions
                objectName: "recordingBarActions"
                anchors.verticalCenter: parent.verticalCenter
                spacing: CelestinaTheme.spaceXs

                CelestinaButton {
                    objectName: "recordingBarStop"
                    role: CelestinaButton.Primary
                    backgroundRadius: CelestinaTheme.radiusPill
                    focusPolicy: Qt.NoFocus
                    text: qsTr("Detener")
                    Accessible.description: qsTr("Detiene la grabación y la guarda")
                    onClicked: SelenitaController.stopRecording()
                }

                CelestinaButton {
                    objectName: "recordingBarDiscard"
                    role: CelestinaButton.Destructive
                    backgroundRadius: CelestinaTheme.radiusPill
                    focusPolicy: Qt.NoFocus
                    text: qsTr("Descartar")
                    Accessible.description: qsTr("Detiene la grabación y borra el archivo")
                    onClicked: SelenitaController.discardRecording()
                }
            }
        }
    }
}
