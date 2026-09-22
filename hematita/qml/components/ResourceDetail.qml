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
            Text {
                text: detail.subtitle
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                elide: Text.ElideLeft
                Layout.maximumWidth: detail.width / 2
            }
        }

        HistoryGraph {
            Layout.fillWidth: true
            Layout.fillHeight: true
            series: detail.series
            load: detail.load
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
