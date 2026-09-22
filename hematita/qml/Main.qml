import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.hematita 1.0
import "components"

// Hematita's window. The sections live in a centred pill strip at the top;
// each page below owns one region and reaches nothing outside itself.
ApplicationWindow {
    id: window

    required property bool reducedMotion
    // Set only by `scripts/smoke.sh`: see the shape gate below.
    required property bool smokeShape
    property bool shapePrinted: false

    width: 1000
    height: 680
    minimumWidth: 640
    minimumHeight: 420
    visible: true
    color: CelestinaTheme.canvas
    title: "Hematita"

    // The sections, in the order the strip shows them. Only Performance has a
    // page in H1; the others are named so the strip is the final one and the
    // pages arrive under it in H3 and H4.
    readonly property var sections: [
        { key: "performance", icon: "gauge", label: qsTr("Rendimiento") },
        { key: "processes", icon: "view-list", label: qsTr("Procesos") },
        { key: "applications", icon: "app-window", label: qsTr("Aplicaciones") },
        { key: "sensors", icon: "zap", label: qsTr("Sensores") }
    ]
    property int currentSection: 0

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: CelestinaTheme.windowMargin
        spacing: CelestinaTheme.spaceLg

        NavStrip {
            id: navStrip
            Layout.alignment: Qt.AlignHCenter
            model: window.sections
            currentIndex: window.currentSection
            onActivated: function(index) { window.currentSection = index }
        }

        HematitaResources {
            id: machine

            // The smoke's shape gate: with HEMATITA_SMOKE_SHAPE set, print one
            // row's contract once, so a nested list that failed to convert is
            // a visible failure rather than an empty graph nobody saw.
            onRevisionChanged: {
                if (!window.smokeShape || machine.revision < 3 || window.shapePrinted)
                    return
                window.shapePrinted = true
                const numbers = machine.resourceNumbers
                const histories = machine.resourceHistories
                console.info("hematita-shape", machine.resourceKinds[0],
                             numbers.length > 0 ? numbers[0].length : -1,
                             histories.length > 0 ? histories[0].length : -1)
            }
        }

        HematitaProcesses {
            id: processHub
        }

        HematitaActivation {
            id: activation
            onRaiseRequested: {
                window.show()
                window.raise()
                window.requestActivate()
            }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: window.currentSection

            PerformancePage {
                metrics: machine
            }

            ProcessPage {
                processes: processHub
                backdrop: window.contentItem
            }

            ApplicationsPage {
                processes: processHub
                backdrop: window.contentItem
            }

            // Sensors arrives in H4.
            Item {
                Text {
                    anchors.centerIn: parent
                    text: qsTr("Esta sección llega en una fase posterior")
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowTitle
                }
            }
        }
    }

    // The two process pages share one hub, so the grouping is the window's to
    // set: whichever page is showing decides it, and the other is rebuilt
    // when it comes back.
    onCurrentSectionChanged: {
        processHub.grouped = window.currentSection === 2
        processHub.refresh()
    }

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = window.reducedMotion
        navStrip.forceActiveFocus()
        machine.start()
        processHub.start()
        activation.start()
    }

    // The sampler thread outlives no window: it is asked to stop and joined
    // before the objects its snapshots are queued to go away.
    Component.onDestruction: machine.shutdown()
}
