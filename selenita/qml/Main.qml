import QtQuick
import QtQuick.Controls
import org.celestina.selenita 1.0
import "components"

// Selenita's window: one window, no strip, three cards stacked (design §5.3).
// In the skeleton it holds the backdrop, the appearance binding, the
// activation route and the three cards, each with its muted line; the
// capture controls arrive in SEL-1-A, recording in SEL-1-B.
ApplicationWindow {
    id: window

    // The activation adapter, for the route below and for the tests.
    readonly property alias activation: activationAdapter

    width: 560
    height: 640
    minimumWidth: 420
    minimumHeight: 420
    visible: true
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

    // A second launch brings this window forward.
    SelenitaActivation {
        id: activationAdapter
    }

    ActivationRoute {
        source: activationAdapter
        host: window
    }

    Column {
        id: cards
        objectName: "cards"
        x: CelestinaTheme.windowMargin
        y: CelestinaTheme.windowMargin
        width: parent.width - CelestinaTheme.windowMargin * 2
        spacing: CelestinaTheme.spaceXl

        SectionCard {
            id: captureCard
            objectName: "captureCard"
            width: cards.width
            title: qsTr("Captura")

            EmptyLine {
                inset: captureCard.rowInset
                text: qsTr("La captura de pantalla llega pronto.")
            }
        }

        SectionCard {
            id: recordingCard
            objectName: "recordingCard"
            width: cards.width
            title: qsTr("Grabación")

            EmptyLine {
                inset: recordingCard.rowInset
                text: qsTr("La grabación de pantalla llega pronto.")
            }
        }

        SectionCard {
            id: historyCard
            objectName: "historyCard"
            width: cards.width
            title: qsTr("Historial")

            EmptyLine {
                inset: historyCard.rowInset
                text: qsTr("Aún no hay capturas.")
            }
        }
    }

    // Development only: the smoke reads what the window shows once it is up.
    Timer {
        interval: 2000
        running: SelenitaController.smokeReport
        onTriggered: console.log("selenita-smoke:"
                                 + " cards=" + [captureCard, recordingCard, historyCard]
                                       .filter(card => card.visible).length
                                 + " fake=" + SelenitaController.fake
                                 + " textScale=" + CelestinaTheme.textScale
                                 + " fontBody=" + CelestinaTheme.fontBody)
    }

    Component.onCompleted: activationAdapter.start()
}
