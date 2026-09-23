pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.hematita 1.0

// One small graph per core, in a grid that fills the detail. The number of
// columns follows the width so eight cores read as two rows of four and
// sixteen as four of four.
Item {
    id: grid

    // A list of minute-long fraction series, one per core.
    required property var histories
    required property string load

    readonly property int columns: Math.max(1, Math.min(grid.histories.length,
                                                        Math.floor(grid.width / (CelestinaTheme.controlHeightXl * 3))))

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Historial por núcleo")

    GridLayout {
        anchors.fill: parent
        columns: grid.columns
        columnSpacing: CelestinaTheme.spaceSm
        rowSpacing: CelestinaTheme.spaceSm

        Repeater {
            model: grid.histories.length

            HistoryGraph {
                required property int index
                Layout.fillWidth: true
                Layout.fillHeight: true
                series: grid.histories[index]
                load: grid.load
                kind: "cpu"
            }
        }
    }
}
