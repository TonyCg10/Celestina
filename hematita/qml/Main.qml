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
    // Set only by `scripts/smoke.sh`: see the section walk below.
    required property bool smokeSections
    property bool shapePrinted: false

    width: 1000
    height: 680
    minimumWidth: 640
    minimumHeight: 420
    visible: true
    color: CelestinaTheme.canvas
    title: "Hematita"

    // The sections, in the order the strip shows them, each with the page
    // under it: Performance, the two process pages that share one hub, and
    // Sensors.
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

        HematitaSensors {
            id: sensorHub
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

            SensorsPage {
                sensors: sensorHub
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

    // The smoke's section walk. A `StackLayout` builds only the page it is
    // showing, so a page nobody selected is a page whose delegates were never
    // constructed — and a headless run that never left Performance would pass
    // while any of the other three failed to build. One second apart, this
    // shows every section in turn and then stops; the gate is the absence of
    // QML errors once all four have been up. It prints nothing of its own.
    Timer {
        running: window.smokeSections
        interval: 1000
        repeat: true
        onTriggered: {
            if (window.currentSection + 1 >= window.sections.length) {
                stop()
                return
            }
            window.currentSection = window.currentSection + 1
        }
    }

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = window.reducedMotion
        navStrip.forceActiveFocus()
        machine.start()
        processHub.start()
        sensorHub.start()
        activation.start()
    }

    // The sampler thread outlives no window: it is asked to stop and joined
    // before the objects its snapshots are queued to go away.
    Component.onDestruction: machine.shutdown()
}
