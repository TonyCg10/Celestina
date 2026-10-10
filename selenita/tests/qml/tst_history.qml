import QtQuick
import QtTest 1.3
import org.celestina.selenita 1.0

// The history card over the stub controller: one row per entry, latest
// first, and each row's four actions reach the controller with its id.
TestCase {
    id: testCase
    name: "History"
    visible: true
    when: windowShown
    width: 560
    height: 600

    Component {
        id: historyComponent
        HistoryCard { width: 520 }
    }

    function init() {
        SelenitaController.reset()
        SelenitaController.fileStem = "Captura"
    }

    function findNamed(node, name) {
        if (node.objectName === name)
            return node
        const kids = node.children !== undefined ? node.children : []
        for (let i = 0; i < kids.length; i += 1) {
            const found = findNamed(kids[i], name)
            if (found !== null)
                return found
        }
        return null
    }

    function named(card, name) {
        const item = findNamed(card, name)
        verify(item !== null, name)
        return item
    }

    function test_an_empty_history_says_so() {
        const card = createTemporaryObject(historyComponent, testCase)
        compare(card.count, 0)
        verify(named(card, "historyEmpty").visible)
    }

    function test_rows_come_latest_first_with_their_details() {
        const card = createTemporaryObject(historyComponent, testCase)
        SelenitaController.capture(true, "#3e91ff", "#40000000")
        SelenitaController.capture(true, "#3e91ff", "#40000000")
        compare(card.count, 2)
        const first = named(card, "historyRow0")
        compare(first.entry.name, "Captura 2.png")
        verify(named(first, "details").text.indexOf("70 B") >= 0)
        compare(named(card, "historyRow1").entry.name, "Captura 1.png")
    }

    function test_row_actions_reach_the_controller() {
        const card = createTemporaryObject(historyComponent, testCase)
        SelenitaController.capture(true, "#3e91ff", "#40000000")
        const row = named(card, "historyRow0")
        const id = row.entry.id
        SelenitaController.calls = []
        mouseClick(named(row, "openAction"))
        mouseClick(named(row, "copyAction"))
        mouseClick(named(row, "showAction"))
        compare(SelenitaController.calls, ["openInFluorita:" + id, "copy:" + id,
                                           "showInSiderita:" + id])
        mouseClick(named(row, "deleteAction"))
        compare(SelenitaController.calls[3], "deleteEntry:" + id)
        compare(card.count, 0)
    }
}
