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

            // Processes, applications and sensors arrive in H3 and H4.
            Repeater {
                model: window.sections.length - 1

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
    }

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = window.reducedMotion
        navStrip.forceActiveFocus()
        machine.start()
        activation.start()
    }
}
