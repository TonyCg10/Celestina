import QtQuick
import QtTest
import CelestinaStyle

// Three rows biggest first; the arrows move the cursor, Enter enters,
// Backspace asks to go up, Space and Delete are left to the host, and an
// empty list says the host's empty text.
TestCase {
    id: testCase
    name: "CelestinaUsageList"
    when: windowShown
    visible: true
    width: 400
    height: 300

    property var chosenIds: []
    property var enteredIds: []
    property int ups: 0
    property int spaces: 0
    property int deletes: 0

    readonly property var threeRows: [
        { id: 10, name: "a", kind: "dir", tone: "dir", share: 0.6, size: "6 MB", percent: "60 %", detail: "" },
        { id: 11, name: "b", kind: "file", tone: "file", share: 0.3, size: "3 MB", percent: "30 %", detail: "x" },
        { id: 12, name: "c", kind: "file", tone: "file", share: 0.1, size: "1 MB", percent: "10 %", detail: "" }
    ]

    Item {
        anchors.fill: parent
        Keys.onSpacePressed: testCase.spaces++
        Keys.onDeletePressed: testCase.deletes++

        CelestinaUsageList {
            id: list
            anchors.fill: parent
            usageRows: testCase.threeRows
            toneColors: ({ dir: CelestinaTheme.glyphAccentBlue, file: CelestinaTheme.glyphAccentViolet })
            emptyText: "empty"
            onChosen: function(id) { testCase.chosenIds = testCase.chosenIds.concat([id]) }
            onEntered: function(id) { testCase.enteredIds = testCase.enteredIds.concat([id]) }
            onUpRequested: testCase.ups++
        }
    }

    function init() {
        CelestinaTheme.reducedMotion = true
        list.usageRows = testCase.threeRows
        list.reset(function() {})
        testCase.chosenIds = []
        testCase.enteredIds = []
        testCase.ups = 0
        testCase.spaces = 0
        testCase.deletes = 0
        list.takeFocus()
        wait(0)
    }

    function test_down_chooses_the_next_row() {
        keyClick(Qt.Key_Down)
        compare(testCase.chosenIds, [11])
        compare(list.currentId(), 11)
    }

    function test_return_enters_the_current_row() {
        keyClick(Qt.Key_Down)
        keyClick(Qt.Key_Return)
        compare(testCase.enteredIds, [11])
    }

    function test_backspace_goes_up() {
        keyClick(Qt.Key_Backspace)
        compare(testCase.ups, 1)
    }

    function test_space_and_delete_reach_the_host() {
        keyClick(Qt.Key_Space)
        compare(testCase.spaces, 1)
        keyClick(Qt.Key_Delete)
        compare(testCase.deletes, 1)
    }

    function test_empty_rows_show_the_empty_text() {
        list.usageRows = []
        wait(0)
        const emptyLabel = findEmptyLabel()
        verify(emptyLabel !== null, "no visible empty text")
        compare(emptyLabel.text, "empty")
    }

    function findEmptyLabel() {
        const label = findChild(list, "emptyText")
        return label !== null && label.visible ? label : null
    }

    function test_focused_row_is_announced() {
        keyClick(Qt.Key_Down)
        const focused = list.contentItem.children[0].currentItem
        verify(focused.activeFocus, "the announced row does not hold focus")
        compare(focused.Accessible.role, Accessible.ListItem)
        verify(focused.Accessible.name.indexOf("b") === 0, focused.Accessible.name)
    }

    function test_list_is_inset_by_space_sm() {
        const view = list.contentItem.children[0]
        const origin = view.mapToItem(list, 0, 0)
        verify(origin.x >= CelestinaTheme.spaceSm, "list x " + origin.x)
        verify(origin.y >= CelestinaTheme.spaceSm, "list y " + origin.y)
        compare(list.radius, CelestinaTheme.radiusMd)
    }

    function test_a_separated_size_is_shown_whole() {
        list.usageRows = [{ id: 20, name: "big", kind: "dir", tone: "dir", share: 1,
                            size: "1.234,5 GiB", percent: "100 %", detail: "" }]
        list.reset(function() {})
        wait(0)
        const view = list.contentItem.children[0]
        const size = findChild(view.itemAtIndex(0), "sizeText")
        verify(size !== null, "no size text")
        compare(size.text, "1.234,5 GiB")
        verify(!size.truncated, "the size is cut")
        verify(size.contentWidth <= size.width, "the size overflows its column")
    }
}
