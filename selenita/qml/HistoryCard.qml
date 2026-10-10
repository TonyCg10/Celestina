pragma ComponentBehavior: Bound
import QtQuick
import org.celestina.selenita 1.0
import "components"

// The history card: the latest captures first, each row with its actions;
// a muted line while there are none. A row is a Tab stop; Up and Down walk
// the rows, Home and End reach the ends, Enter opens the row's file and
// Delete moves it to the trash, the focus landing on the row that takes its
// place.
SectionCard {
    id: card

    readonly property int count: SelenitaController.history.length
    // The row to focus once the history has lost one by the keyboard; -1
    // when none is owed. While it stands, a trash is in flight and a second
    // Delete is ignored; it is dropped when the trash fails or the focus
    // moves to a control outside the card.
    property int owedIndex: -1
    readonly property Item focusItem: card.Window.window ? card.Window.window.activeFocusItem
                                                         : null

    // Whether the item sits in this card.
    function holds(item: Item): bool {
        let node = item
        while (node && node !== card)
            node = node.parent
        return node === card
    }

    // Puts the keyboard focus on the row at `index`, clamped to the rows.
    function focusRow(index: int) {
        if (card.count === 0)
            return
        const row = rowRepeater.itemAt(Math.max(0, Math.min(index, card.count - 1)))
        if (row)
            row.forceActiveFocus(Qt.TabFocusReason)
    }

    function settleOwedFocus() {
        const index = card.owedIndex
        card.owedIndex = -1
        if (index >= 0)
            card.focusRow(index)
    }

    title: qsTr("Historial")

    // The rows are rebuilt when the history changes; the owed focus waits
    // for the new ones.
    onCountChanged: {
        if (card.owedIndex >= 0)
            Qt.callLater(card.settleOwedFocus)
    }

    // A rebuilt row hands the focus to the window's scope, which is no Tab
    // stop; a Tab stop outside the card is a person who moved on.
    onFocusItemChanged: {
        if (card.focusItem && card.focusItem.activeFocusOnTab && !card.holds(card.focusItem))
            card.owedIndex = -1
    }

    Connections {
        target: SelenitaController

        function onNotice(kind: string, text: string) {
            if (kind === "error")
                card.owedIndex = -1
        }
    }

    EmptyLine {
        objectName: "historyEmpty"
        inset: card.rowInset
        visible: card.count === 0
        text: qsTr("Aún no hay capturas.")
    }

    Repeater {
        id: rowRepeater
        objectName: "historyRows"
        model: SelenitaController.history

        delegate: HistoryRow {
            required property var modelData
            required property int index

            objectName: "historyRow" + index
            entry: modelData
            inset: card.rowInset
            separated: index > 0
            onWalkRequested: delta => card.focusRow(index + delta)
            onEdgeRequested: last => card.focusRow(last ? card.count - 1 : 0)
            onTrashRequested: {
                if (card.owedIndex >= 0)
                    return
                card.owedIndex = index
                SelenitaController.deleteEntry(entry.id)
            }
        }
    }
}
