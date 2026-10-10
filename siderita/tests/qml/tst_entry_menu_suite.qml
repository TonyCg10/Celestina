import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The «Abrir en» section of an entry's menu: it lists what the controller says
// applies to the selection, one send entry per connected phone, and vanishes
// when nothing does. The controller is a stub: this tests the menu, not the
// decision (the `apps.rs` tests own that).
TestCase {
    id: testCase
    name: "EntryMenuSuite"
    width: 400
    height: 900
    visible: true
    when: windowShown

    QtObject {
        id: controllerStub

        property bool trashActive: false
        property bool showHidden: false
        property int phoneRevision: 0
        property var phoneNames: ["Galaxy", "Pixel", "Tablet"]
        // id and connected flag, as `phoneInfo` answers them: the tablet is
        // paired but offline, so it never gets a send entry.
        property var phones: [["g1", "1"], ["p2", "1"], ["t3", "0"]]

        function phoneInfo(index) {
            return [phones[index][0], phoneNames[index], "phone", phones[index][1]]
        }
        function indexForToken(token) { return Number(token) }
        function entryPath(index) { return "/home/toni/entry" + index }
        function entryTargetsDirectory(index) { return index === 7 }
        function areArchives(keys) { return false }
    }

    QtObject {
        id: suiteStub

        property var answer: []
        property var asked: []
        property var opened: []
        property var sent: []

        function targets(keys, folders) {
            asked = [keys, folders]
            return answer
        }
        function open(target, keys) { opened.push({ target: target, keys: keys }) }
        function send(device, keys) { sent.push({ device: device, keys: keys }) }
    }

    QtObject {
        id: iconsStub
        function isFavorite(path) { return false }
        function customIconAccent(path) { return "" }
    }

    QtObject {
        id: panelStub
        property var icons: iconsStub
        property var selectedTokens: ({})
        function actingCount(token) { return 1 }
        function operativePaths(token, path) { return [path] }
    }

    EntryContextMenu {
        id: menu
        backdropSource: testCase
        controller: controllerStub
        panel: panelStub
        suite: suiteStub
        targetToken: "1"
        targetName: "entry1"
        targetPath: "/home/toni/entry1"
    }

    function init() {
        menu.close()
        tryCompare(menu, "visible", false)
        suiteStub.opened = []
        suiteStub.sent = []
        controllerStub.phones = [["g1", "1"], ["p2", "1"], ["t3", "0"]]
    }

    function shown(prefix) {
        const found = []
        for (let index = 0; index < menu.count; ++index) {
            const item = menu.itemAt(index)
            if (item && item.objectName.startsWith(prefix) && item.visible)
                found.push(item)
        }
        return found
    }

    function setAnswer(targets) {
        suiteStub.answer = targets
        controllerStub.phoneRevision++
        menu.popup(0, 0)
        tryCompare(menu, "opened", true)
    }

    function test_the_suite_hears_each_acting_entry_and_whether_it_is_a_folder() {
        setAnswer([])
        compare(suiteStub.asked[0], ["/home/toni/entry1"])
        compare(suiteStub.asked[1], ["0"])
    }

    function test_a_file_lists_grafita_and_one_send_entry() {
        controllerStub.phones = [["g1", "1"], ["p2", "0"], ["t3", "0"]]
        setAnswer(["grafita", "phone"])
        const entries = shown("suiteTarget:")
        compare(entries.length, 2)
        compare(entries[0].text, "Grafita")
        compare(entries[1].text, qsTr("Enviar al móvil"))
        compare(shown("suiteSection").length, 1)
    }

    function test_the_entries_follow_the_section_header() {
        setAnswer(["fluorita", "hematita"])
        const header = menu.sectionIndex()
        compare(menu.itemAt(header + 1).objectName, "suiteTarget:fluorita")
        compare(menu.itemAt(header + 2).objectName, "suiteTarget:hematita")
    }

    function test_a_pdf_lists_calcita_between_grafita_and_the_send_entry() {
        controllerStub.phones = [["g1", "1"], ["p2", "0"], ["t3", "0"]]
        setAnswer(["grafita", "calcita", "phone"])
        const entries = shown("suiteTarget:")
        compare(entries.length, 3)
        compare(entries[1].objectName, "suiteTarget:calcita")
        compare(entries[1].text, "Calcita")
        compare(entries[1].icon.name, "org.celestina.Calcita")
        entries[1].triggered()
        compare(suiteStub.opened.length, 1)
        compare(suiteStub.opened[0].target, "calcita")
        compare(suiteStub.opened[0].keys, ["/home/toni/entry1"])
    }

    function test_each_connected_phone_gets_an_entry() {
        setAnswer(["grafita", "phone"])
        const entries = shown("suiteTarget:phone")
        compare(entries.length, 2)
        compare(entries[0].text, "Enviar a Galaxy")
        compare(entries[1].text, "Enviar a Pixel")
        entries[1].triggered()
        compare(suiteStub.sent.length, 1)
        compare(suiteStub.sent[0].device, "p2")
        compare(suiteStub.sent[0].keys, ["/home/toni/entry1"])
    }

    function test_no_connected_phone_means_no_send_entry() {
        controllerStub.phones = [["g1", "0"], ["p2", "0"], ["t3", "0"]]
        setAnswer(["grafita", "phone"])
        compare(shown("suiteTarget:phone").length, 0)
        compare(shown("suiteTarget:").length, 1)
    }

    function test_an_application_entry_opens_through_the_suite() {
        setAnswer(["grafita"])
        shown("suiteTarget:grafita")[0].triggered()
        compare(suiteStub.opened.length, 1)
        compare(suiteStub.opened[0].target, "grafita")
        compare(suiteStub.opened[0].keys, ["/home/toni/entry1"])
    }

    function test_nothing_that_applies_hides_the_section() {
        setAnswer([])
        compare(shown("suiteTarget:").length, 0)
        compare(shown("suiteSection").length, 0)
    }
}
