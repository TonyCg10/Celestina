// language-contract: allow-non-english (the Spanish menu entry, compared verbatim)
import QtQuick
import QtTest 1.3
import org.celestina.calcita 1.0

// The recents card of the empty state: a document opened once is listed,
// a click on its row opens it, and its row menu's «Quitar de recientes»
// takes it off the list.
TestCase {
    id: testCase
    name: "Recents"
    when: windowShown

    readonly property string fixture: String(Qt.resolvedUrl("../fixtures/three-pages.pdf"))
                                      .substring("file://".length)

    Component {
        id: mainComponent
        Main {}
    }

    function findNamed(node, name) {
        if (node.objectName === name)
            return node
        const kids = node.children !== undefined ? node.children : []
        let found = null
        let i = 0
        while (found === null && i < kids.length) {
            found = findNamed(kids[i], name)
            i += 1
        }
        return found
    }

    function init() {
        CalcitaController.reset()
    }

    function test_an_opened_document_is_listed_and_forgotten_from_its_menu() {
        const window = createTemporaryObject(mainComponent, testCase)
        window.activation.openRequested([testCase.fixture])
        const document = window.documentWindows.objectAt(0)
        tryVerify(() => document.reader.loaded, 5000)
        compare(CalcitaController.recents, [testCase.fixture], "opening remembers it")
        document.close()
        tryCompare(window.documentWindows, "count", 0)

        const empty = findNamed(window.contentItem, "emptyState")
        const card = findNamed(empty, "recentCard")
        verify(card.visible, "the card lists it")
        const row = findNamed(empty, "recentRow0")
        verify(row)
        compare(row.Accessible.name, "Abrir three-pages.pdf")

        // The window retired when the document opened; show it to click.
        window.retired = false
        tryVerify(() => window.visible)
        waitForRendering(empty)
        mouseClick(row, row.width / 2, row.height / 2, Qt.RightButton)
        const menu = empty.recentMenu
        tryVerify(() => menu.opened)
        const item = findNamed(menu.contentItem, "forgetRecentItem")
            ?? menu.itemAt(0)
        compare(item.text, "Quitar de recientes")
        item.triggered()
        compare(CalcitaController.recents, [])
        tryVerify(() => !card.visible, 2000, "the card empties")
        compare(window.documentWindows.count, 0, "forgetting opens nothing")
    }

    function test_a_click_on_a_recent_row_opens_it() {
        CalcitaController.remember(testCase.fixture, 1, "width", false)
        const window = createTemporaryObject(mainComponent, testCase)
        tryVerify(() => window.visible)
        const empty = findNamed(window.contentItem, "emptyState")
        const row = findNamed(empty, "recentRow0")
        waitForRendering(empty)
        mouseClick(row)
        compare(window.documentWindows.count, 1)
    }

    function test_the_menu_key_opens_the_row_menu() {
        CalcitaController.remember(testCase.fixture, 1, "width", false)
        const window = createTemporaryObject(mainComponent, testCase)
        tryVerify(() => window.visible)
        window.requestActivate()
        tryVerify(() => window.active)
        const empty = findNamed(window.contentItem, "emptyState")
        const row = findNamed(empty, "recentRow0")
        row.forceActiveFocus()
        keyClick(Qt.Key_F10, Qt.ShiftModifier)
        tryVerify(() => empty.recentMenu.opened)
        empty.recentMenu.close()
    }
}
