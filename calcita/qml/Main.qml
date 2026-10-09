import QtQuick
import QtQuick.Controls
import org.celestina.calcita 1.0
import "components"

// Calcita's window: one document per window. In the skeleton it holds the
// backdrop, the appearance binding, the activation route and the empty state;
// the page area and its bar arrive in CAL-1-A.
ApplicationWindow {
    id: window

    // The activation adapter, for the route below and for the tests.
    readonly property alias activation: activationAdapter

    width: 900
    height: 680
    minimumWidth: 480
    minimumHeight: 360
    visible: true
    // Transparent: the compositor blurs what lies behind the window and
    // CelestinaBackdrop lays the canvas over it (DESIGN §5.2 L0).
    color: CelestinaTheme.clear
    title: "Calcita"

    CelestinaBackdrop {
        anchors.fill: parent
    }

    // The suite's appearance file (reduced motion, text scale), followed by
    // the controller and bound into the theme once for this window.
    CelestinaAppearance {
        reducedMotion: CalcitaController.appearanceReducedMotion
        textScale: CalcitaController.appearanceTextScale
    }

    // A second launch brings this window forward.
    CalcitaActivation {
        id: activationAdapter
    }

    ActivationRoute {
        source: activationAdapter
        host: window
    }

    EmptyState {
        id: emptyState
        anchors.centerIn: parent
        width: Math.min(parent.width - CelestinaTheme.windowMargin * 2, 360)
    }

    // Development only: the smoke reads what the window shows once it is up.
    Timer {
        interval: 2000
        running: CalcitaController.smokeReport
        onTriggered: console.log("calcita-smoke:"
                                 + " empty=" + emptyState.visible
                                 + " textScale=" + CelestinaTheme.textScale
                                 + " fontBody=" + CelestinaTheme.fontBody)
    }

    Component.onCompleted: activationAdapter.start()
}
