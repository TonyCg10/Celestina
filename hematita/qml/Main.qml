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
    property bool sensorShapePrinted: false
    property bool serviceShapePrinted: false
    property bool storageShapePrinted: false

    width: 1000
    height: 680
    minimumWidth: 640
    minimumHeight: 420
    visible: true
    color: CelestinaTheme.canvas
    title: "Hematita"

    // The sections, in the order the strip shows them, each with the page
    // under it: Performance, the two process pages that share one hub,
    // Sensors, Services and Storage.
    readonly property var sections: [
        { key: "performance", icon: "gauge", label: qsTr("Rendimiento") },
        { key: "processes", icon: "view-list", label: qsTr("Procesos") },
        { key: "applications", icon: "app-window", label: qsTr("Aplicaciones") },
        { key: "sensors", icon: "zap", label: qsTr("Sensores") },
        { key: "services", icon: "toolbox", label: qsTr("Servicios") },
        { key: "storage", icon: "hard-drive", label: qsTr("Almacenamiento") }
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

            // The sensors' own shape gate, printed once the section walk has
            // reached Sensores and a reading has landed. A page that built
            // without error but published no chip is the failure no error
            // message would have named, so the smoke reads the two counts.
            onRevisionChanged: window.printSensorShape()
        }

        HematitaServices {
            id: serviceHub

            // The services' own shape gate, printed once the walk has reached
            // Servicios and a listing has landed. A page that built without
            // error but listed no unit is the failure no error message would
            // have named.
            onRevisionChanged: window.printServiceShape()
        }

        HematitaAnalysis {
            id: analysisHub

            // The storage section's own shape gate, printed once the walk has
            // reached Almacenamiento and the locations have landed. A page
            // that built without error but found no mount is the failure no
            // error message would have named.
            onRevisionChanged: window.printStorageShape()
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

            ServicesPage {
                services: serviceHub
                backdrop: window.contentItem
            }

            StoragePage {
                analysis: analysisHub
                backdrop: window.contentItem
            }
        }
    }

    // The two process pages share one hub, so the grouping is the window's to
    // set: whichever page is showing decides it, and the other is rebuilt
    // when it comes back.
    onCurrentSectionChanged: {
        processHub.grouped = window.currentSection === 2
        processHub.refresh()
        // The storage section reads the mounts again each time it is shown:
        // a disk plugged in since is there, and nothing is read while the
        // section is not up.
        if (window.currentSection === 5)
            analysisHub.open()
    }

    function printSensorShape() {
        if (!window.smokeSections || window.sensorShapePrinted
            || window.currentSection !== 3 || sensorHub.revision < 1)
            return
        window.sensorShapePrinted = true
        console.info("hematita-sensors", sensorHub.chipKeys.length,
                     sensorHub.channelKinds.length)
    }

    function printServiceShape() {
        if (!window.smokeSections || window.serviceShapePrinted
            || window.currentSection !== 4 || serviceHub.revision < 1)
            return
        window.serviceShapePrinted = true
        console.info("hematita-services", serviceHub.shownCount,
                     serviceHub.systemAvailable, serviceHub.userAvailable)
    }

    function printStorageShape() {
        if (!window.smokeSections || window.storageShapePrinted
            || window.currentSection !== 5 || analysisHub.mode !== "locations"
            || analysisHub.revision < 1 || analysisHub.busy)
            return
        window.storageShapePrinted = true
        console.info("hematita-storage", analysisHub.locationNames.length)
    }

    // The smoke's section walk. A `StackLayout` builds only the page it is
    // showing, so a page nobody selected is a page whose delegates were never
    // constructed — and a headless run that never left Performance would pass
    // while any of the others failed to build. One second apart, this
    // shows every section in turn and then stops; the gate is the absence of
    // QML errors once all six have been up. It prints nothing of its own.
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
            // A reading usually landed long before the walk arrives, so the
            // next `revisionChanged` is not what to wait for.
            window.printSensorShape()
            window.printServiceShape()
            window.printStorageShape()
        }
    }

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = window.reducedMotion
        navStrip.forceActiveFocus()
        machine.start()
        processHub.start()
        sensorHub.start()
        serviceHub.start()
        activation.start()
    }

    // The sampler thread outlives no window: it is asked to stop and joined
    // before the objects its snapshots are queued to go away.
    Component.onDestruction: machine.shutdown()
}
