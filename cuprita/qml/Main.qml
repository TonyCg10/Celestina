import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
// The list models derive from QAbstractListModel, which qmllint resolves only
// through an explicit QtQml.Models import (the module's `depends` is not read).
import QtQml.Models
import org.celestina.cuprita 1.0
import "components"
import "pages"

// Cuprita's window. The sections live in a centred pill strip at the top;
// each page below owns one region and reaches nothing outside itself.
ApplicationWindow {
    id: window

    width: 880
    height: 640
    minimumWidth: 560
    minimumHeight: 420
    visible: true
    // Transparent: the compositor blurs what lies behind the window and
    // CelestinaBackdrop paints the Haze canvas over it (DESIGN §5.2 L0).
    color: CelestinaTheme.clear
    title: "Cuprita"

    property int currentSection: 0

    CelestinaBackdrop {
        anchors.fill: parent
    }

    // The suite's appearance file (reduced motion, text scale), followed on
    // the controller's worker and bound into the theme once for this window.
    CelestinaAppearance {
        reducedMotion: CupritaController.appearanceReducedMotion
        textScale: CupritaController.appearanceTextScale
    }

    // A second launch asks the running window to come to the front.
    CupritaActivation {
        id: activation
    }

    ActivationRoute {
        source: activation
        host: window
    }

    // Ctrl+1, Ctrl+2 and Ctrl+3 jump to a section from anywhere.
    SectionShortcuts {
        onActivated: function(index) { window.currentSection = index }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: CelestinaTheme.windowMargin
        spacing: CelestinaTheme.spaceLg

        NavStrip {
            id: navStrip
            Layout.alignment: Qt.AlignHCenter
            model: Sections.all
            currentIndex: window.currentSection
            onActivated: function(index) { window.currentSection = index }
        }

        StackLayout {
            id: pages
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: window.currentSection

            NetworkPage {
                controller: NetworkController
                networks: NetworkModel { id: networkModel }
                backdropSource: pages
            }
            BluetoothPage {
                controller: BluetoothController
                devices: DeviceModel { id: deviceModel }
                backdropSource: pages
            }
            AudioPage {
                controller: AudioController
                sinks: EndpointModel { id: sinkModel; kind: "sink" }
                sources: EndpointModel { id: sourceModel; kind: "source" }
                streams: StreamModel { id: streamModel }
                backdropSource: pages
            }
        }
    }

    // The three controllers' notices, one pill for the window.
    NoticePill {
        id: notice
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.windowMargin
    }

    Connections {
        target: NetworkController
        function onNotice(kind, text) { notice.show(kind, text) }
    }
    Connections {
        target: BluetoothController
        function onNotice(kind, text) { notice.show(kind, text) }
    }
    Connections {
        target: AudioController
        function onNotice(kind, text) { notice.show(kind, text) }
    }

    // Development only: the smoke reads the models' rows once they are filled.
    Timer {
        interval: 3000
        running: CupritaController.smokeReport
        onTriggered: console.log("cuprita-smoke:"
                                 + " networks=" + networkModel.count
                                 + " devices=" + deviceModel.count
                                 + " endpoints=" + (sinkModel.count + sourceModel.count)
                                 + " streams=" + streamModel.count
                                 + " sink=" + sinkModel.defaultId
                                 + " source=" + sourceModel.defaultId
                                 + " textScale=" + CelestinaTheme.textScale
                                 + " fontBody=" + CelestinaTheme.fontBody)
    }

    // Development only, behind the same switch: each change of the theme's
    // text scale, as the appearance file reaches it.
    Connections {
        target: CelestinaTheme
        enabled: CupritaController.smokeReport
        function onFontBodyChanged() {
            console.log("cuprita-appearance: textScale=" + CelestinaTheme.textScale
                        + " fontBody=" + CelestinaTheme.fontBody)
        }
    }

    Component.onCompleted: {
        activation.start()
        navStrip.forceActiveFocus()
    }
}
