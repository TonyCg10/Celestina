import QtQuick
import org.celestina.selenita 1.0
import "components"

// The recording card (design §5.3): record or stop, with a red dot and the
// elapsed time while it records; the system's sound switch; what the host
// lacks when it cannot record; the last recording and «Abrir en Fluorita».
// The state lives in the controller; this card only shows it and asks.
SectionCard {
    id: card

    readonly property string phase: SelenitaController.recordingState
    readonly property bool recording: card.phase === "recording"
    // Between the ask and the answer: the portal's dialog, or the muxer
    // finishing the file.
    readonly property bool inTransit: card.phase === "preparing" || card.phase === "stopping"
    readonly property bool unavailable: SelenitaController.recorderMissing !== ""
    // Whole seconds since the recording began, kept by the clock the
    // recording bar shares.
    readonly property alias elapsed: clock.elapsed

    // `m:ss`, the hours only once there are some.
    function elapsedText(seconds: int): string {
        return clock.format(seconds)
    }

    title: qsTr("Grabación")

    RecordingClock {
        id: clock
        active: card.recording
    }

    // The dot, the time and the button.
    Item {
        width: parent.width
        height: CelestinaTheme.controlHeightLg + CelestinaTheme.spaceSm * 2

        Rectangle {
            id: dot
            objectName: "recordingDot"
            anchors.left: parent.left
            anchors.leftMargin: card.rowInset
            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.spaceMd
            height: width
            radius: width / 2
            color: CelestinaTheme.danger
            visible: card.recording
            Accessible.ignored: true

            // A slow blink while it records, still under reduced motion.
            SequentialAnimation on opacity {
                running: card.recording && !CelestinaTheme.reducedMotion
                loops: Animation.Infinite
                alwaysRunToEnd: true
                NumberAnimation { to: 0.35; duration: CelestinaTheme.motionSlow }
                NumberAnimation { to: 1; duration: CelestinaTheme.motionSlow }
            }
        }

        Text {
            objectName: "elapsedText"
            anchors.left: dot.visible ? dot.right : parent.left
            anchors.leftMargin: dot.visible ? CelestinaTheme.spaceSm : card.rowInset
            anchors.right: recordButton.left
            anchors.rightMargin: CelestinaTheme.spaceMd
            anchors.verticalCenter: parent.verticalCenter
            text: card.recording ? clock.text
                  : card.phase === "preparing" ? qsTr("Elige la pantalla en el diálogo del portal…")
                  : card.phase === "stopping" ? qsTr("Terminando el archivo…")
                  : card.unavailable ? qsTr("Falta «%1»: instala gst-plugins-good para grabar.")
                                           .arg(SelenitaController.recorderMissing)
                  : qsTr("Graba la pantalla en MP4.")
            elide: Text.ElideRight
            color: card.recording ? CelestinaTheme.text : CelestinaTheme.textMuted
            font.family: card.recording ? CelestinaTheme.monoFamily : CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        CelestinaButton {
            id: recordButton
            objectName: "recordButton"
            anchors.right: parent.right
            anchors.rightMargin: card.rowInset
            anchors.verticalCenter: parent.verticalCenter
            height: CelestinaTheme.controlHeightLg
            role: card.recording ? CelestinaButton.Destructive : CelestinaButton.Primary
            density: CelestinaButton.Regular
            enabled: !card.inTransit && !card.unavailable
            text: card.recording ? qsTr("Detener") : qsTr("Grabar")
            Accessible.description: qsTr("Tecla R")
            onClicked: SelenitaController.toggleRecording()
        }
    }

    SettingRow {
        objectName: "audioRow"
        inset: card.rowInset
        separated: true
        label: qsTr("Con sonido del sistema")
        // Chosen before the recording starts; the pipeline is built then.
        enabled: card.phase === "idle"
        checked: SelenitaController.withAudio
        onToggled: on => SelenitaController.withAudio = on
    }

    // The last recording of this window, and the way to watch it.
    Item {
        objectName: "lastRecordingRow"
        width: parent.width
        height: CelestinaTheme.rowHeight
        visible: SelenitaController.lastRecordingName !== ""

        RowDivider { inset: card.rowInset }

        Text {
            objectName: "lastRecordingName"
            anchors.left: parent.left
            anchors.leftMargin: card.rowInset
            anchors.right: openButton.left
            anchors.rightMargin: CelestinaTheme.spaceMd
            anchors.verticalCenter: parent.verticalCenter
            text: SelenitaController.lastRecordingName
            elide: Text.ElideMiddle
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Última grabación: %1").arg(SelenitaController.lastRecordingName)
        }

        CelestinaButton {
            id: openButton
            objectName: "openLastRecording"
            anchors.right: parent.right
            anchors.rightMargin: card.rowInset
            anchors.verticalCenter: parent.verticalCenter
            text: qsTr("Abrir en Fluorita")
            onClicked: SelenitaController.openInFluorita(SelenitaController.lastRecordingId)
        }
    }
}
