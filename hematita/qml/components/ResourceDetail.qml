pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.hematita 1.0

// The selected resource, large: a title line, the minute graph, and the facts
// beneath it as label/value pairs. The page hands in everything; this draws.
CelestinaSurface {
    id: detail

    required property string title
    required property string subtitle
    required property var series
    required property string load
    // Flat list of alternating label, value strings.
    required property var facts
    // One minute per core when the CPU is shown; empty otherwise.
    required property var coreHistories
    // Why this resource cannot be read, or empty.
    required property string notice

    // The toggle owns its own checked state — `checkable: true` and nothing
    // else, as the shared button's contract says — and this reads it, so
    // nothing ever assigns into `checked` and no binding is destroyed.
    readonly property bool showCores: coreToggle.checked

    role: CelestinaSurface.Grouped
    padding: CelestinaTheme.spaceXl

    contentItem: ColumnLayout {
        spacing: CelestinaTheme.spaceLg

        RowLayout {
            Layout.fillWidth: true
            Text {
                text: detail.title
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontTitle
                font.weight: CelestinaTheme.weightDemiBold
            }
            Item { Layout.fillWidth: true }
            CelestinaIconButton {
                id: coreToggle
                visible: detail.coreHistories.length > 0
                iconName: coreToggle.checked ? "gauge" : "view-grid"
                helpText: coreToggle.checked ? qsTr("Ver una gráfica") : qsTr("Ver por núcleo")
                role: CelestinaButton.Ghost
                checkable: true
            }
            Text {
                text: detail.subtitle
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                elide: Text.ElideLeft
                Layout.maximumWidth: detail.width / 2
            }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: detail.showCores && detail.coreHistories.length > 0 ? 1 : 0

            HistoryGraph {
                series: detail.series
                load: detail.load
            }

            CoreGrid {
                histories: detail.coreHistories
                load: detail.load
            }
        }

        Text {
            Layout.fillWidth: true
            visible: detail.notice.length > 0
            text: detail.notice
            wrapMode: Text.Wrap
            color: CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
        }

        GridLayout {
            Layout.fillWidth: true
            columns: 4
            columnSpacing: CelestinaTheme.space2xl
            rowSpacing: CelestinaTheme.spaceSm

            Repeater {
                model: detail.facts.length

                Text {
                    id: fact
                    required property int index
                    readonly property bool isLabel: fact.index % 2 === 0

                    text: detail.facts[fact.index]
                    color: fact.isLabel ? CelestinaTheme.textFaint : CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: fact.isLabel ? CelestinaTheme.fontCaption
                                                 : CelestinaTheme.fontRowTitle
                    font.features: CelestinaTheme.fontFeaturesTabular
                }
            }
        }
    }
}
