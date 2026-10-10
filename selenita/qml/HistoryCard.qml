pragma ComponentBehavior: Bound
import QtQuick
import org.celestina.selenita 1.0
import "components"

// The history card: the latest captures first, each row with its actions;
// a muted line while there are none.
SectionCard {
    id: card

    readonly property int count: SelenitaController.history.length

    title: qsTr("Historial")

    EmptyLine {
        objectName: "historyEmpty"
        inset: card.rowInset
        visible: card.count === 0
        text: qsTr("Aún no hay capturas.")
    }

    Repeater {
        objectName: "historyRows"
        model: SelenitaController.history

        delegate: HistoryRow {
            required property var modelData
            required property int index

            objectName: "historyRow" + index
            entry: modelData
            inset: card.rowInset
            separated: index > 0
        }
    }
}
