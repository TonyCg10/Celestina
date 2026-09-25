import QtQuick
import QtTest
import CelestinaStyle

// Three tiles in reading order; the arrows walk them, Enter enters, Backspace
// asks to go up, Space is left to the host, the remainder is disabled.
TestCase {
    id: testCase
    name: "CelestinaTreemap"
    when: windowShown
    width: 400
    height: 300

    property var chosenIds: []
    property var enteredIds: []
    property int ups: 0
    property int spaces: 0
    property int deletes: 0

    Item {
        anchors.fill: parent
        Keys.onSpacePressed: testCase.spaces++
        Keys.onDeletePressed: testCase.deletes++

        CelestinaTreemap {
            id: map
            anchors.fill: parent
            tiles: [
                { id: 10, x: 0, y: 0, w: 0.5, h: 1, name: "a", kind: "dir", tone: "dir" },
                { id: 11, x: 0.5, y: 0, w: 0.5, h: 0.5, name: "b", kind: "file", tone: "file" },
                { id: -1, x: 0.5, y: 0.5, w: 0.5, h: 0.5, name: "", kind: "other", tone: "other" }
            ]
            toneColors: ({ dir: CelestinaTheme.glyphAccentBlue, file: CelestinaTheme.glyphAccentViolet, other: CelestinaTheme.textFaint })
            currentId: 10
            currentName: "root"
            onChosen: function(id) { testCase.chosenIds = testCase.chosenIds.concat([id]) }
            onEntered: function(id) { testCase.enteredIds = testCase.enteredIds.concat([id]) }
            onUpRequested: testCase.ups++
        }
    }

    function init() {
        CelestinaTheme.reducedMotion = true
        testCase.chosenIds = []
        testCase.enteredIds = []
        testCase.ups = 0
        testCase.spaces = 0
        testCase.deletes = 0
        map.markedIds = []
        map.dimmedIds = []
        map.contentItem.forceActiveFocus()
        wait(0)
    }

    function tileWithId(id) {
        const children = map.contentItem.children
        for (let index = 0; index < children.length; ++index) {
            if (children[index].tileData !== undefined && children[index].tileData.id === id)
                return children[index]
        }
        return null
    }

    function test_arrows_walk_reading_order() {
        keyClick(Qt.Key_Right)
        compare(testCase.chosenIds, [11])
    }

    function test_enter_enters_and_backspace_goes_up() {
        keyClick(Qt.Key_Return)
        compare(testCase.enteredIds, [10])
        keyClick(Qt.Key_Backspace)
        compare(testCase.ups, 1)
    }

    function test_space_and_delete_reach_the_host() {
        keyClick(Qt.Key_Space)
        compare(testCase.spaces, 1)
        keyClick(Qt.Key_Delete)
        compare(testCase.deletes, 1)
    }

    function test_remainder_is_named_and_disabled() {
        const remainder = tileWithId(-1)
        verify(remainder !== null, "no remainder tile")
        compare(remainder.enabled, false)
        compare(remainder.Accessible.name, "otros")
    }

    function test_tiles_are_named_by_kind() {
        compare(tileWithId(10).Accessible.name, "a, carpeta")
        compare(tileWithId(11).Accessible.name, "b, archivo")
    }

    function test_dimmed_ids_fade() {
        map.dimmedIds = [11]
        compare(tileWithId(11).opacity, CelestinaTheme.unavailableContentOpacity)
        compare(tileWithId(10).opacity, 1)
    }

    function test_dimmed_remainder_fades() {
        map.dimmedIds = [-1]
        compare(tileWithId(-1).opacity, CelestinaTheme.unavailableContentOpacity)
    }

    function test_left_and_up_step_backwards() {
        map.currentId = 11
        keyClick(Qt.Key_Left)
        keyClick(Qt.Key_Up)
        compare(testCase.chosenIds, [10, 10])
        map.currentId = 10
    }
}
