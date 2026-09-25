import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The folder occupation section over a stub hub: what it weaves out of the
// index-aligned lists, and which request each key and button sends. The
// scan itself is SideritaUsage's and is tested in Rust.
TestCase {
    id: testCase
    name: "FolderUsage"
    width: 720
    height: 420
    visible: true
    when: windowShown

    property var enterCalls: []
    property int upCalls: 0
    property var locations: []
    property var hematitaCalls: []
    property int navigations: 0
    property var upToCalls: []
    property var hostKeys: []

    QtObject {
        id: usageStub

        property string owner: "quicklook"
        property string mode: "analysed"
        property string failure: ""
        property string root: "/home"
        property string currentPath: "/home/a"
        property var crumbs: ["home", "a"]
        property real progressEntries: 0
        property real progressBytes: 0
        property real totalBytes: 12288
        property real filesBelow: 3
        property real foldersBelow: 1
        property real unreadableBelow: 0
        property real otherDevices: 0
        property var rowIds: [10, 11, -1]
        property var rowNames: ["x", "y", ""]
        property var rowKinds: ["dir", "file", "other"]
        property var rowAllocated: [8192, 2048, 2048]
        property var rowShares: [0.666, 0.167, 0.167]
        property var rowFiles: [1, 1, 1]
        property var rowUnreadable: [0, 0, 0]
        property var rowMerged: [0, 0, 2]
        property var tiles: [10, 0, 0, 0.5, 1, 11, 0.5, 0, 0.5, 0.5, -1, 0.5, 0.5, 0.5, 0.5]
        property int revision: 1

        function enter(id) {
            testCase.enterCalls.push(id)
            return id === 10
        }
        function up() {
            testCase.upCalls++
            return true
        }
        function upTo(depth) {
            testCase.upToCalls.push(depth)
            return true
        }
        function pathOf(id) { return "/home/a/x" }
        function openInHematita(path) {
            testCase.hematitaCalls.push(path)
            return true
        }
    }

    QtObject {
        id: controllerStub
        function openLocation(path) { testCase.locations.push(path) }
    }

    // Stands in for the host modal: whatever the section leaves unaccepted
    // lands here.
    Item {
        anchors.fill: parent
        Keys.onPressed: function(event) {
            testCase.hostKeys.push(event.key)
            event.accepted = true
        }

        FolderUsage {
            id: section
            anchors.fill: parent
            usage: usageStub
            controller: controllerStub
            onNavigated: testCase.navigations++
        }
    }

    function init() {
        testCase.enterCalls = []
        testCase.upCalls = 0
        testCase.locations = []
        testCase.hematitaCalls = []
        testCase.navigations = 0
        testCase.upToCalls = []
        testCase.hostKeys = []
        usageStub.mode = "analysed"
        usageStub.failure = ""
        usageStub.currentPath = "/home/a"
        usageStub.rowIds = [10, 11, -1]
        usageStub.revision++
    }

    function test_a_the_rows_are_woven_with_the_merged_remainder() {
        compare(section.usageRows.length, 3)
        compare(section.usageRows[0].name, "x")
        compare(section.usageRows[2].name, "otros (2)")
        compare(section.tiles.length, 3)
        compare(section.currentId, 10)
    }

    function test_b_return_on_the_first_row_drills_into_it() {
        const list = findChild(section, "usageList")
        verify(list !== null)
        list.takeFocus()
        keyClick(Qt.Key_Return)
        compare(testCase.enterCalls.length, 1)
        compare(testCase.enterCalls[0], 10)
    }

    function test_c_backspace_asks_to_go_up() {
        findChild(section, "usageList").takeFocus()
        keyClick(Qt.Key_Backspace)
        compare(testCase.upCalls, 1)
    }

    function test_d_ctrl_return_goes_to_the_folder_in_siderita() {
        findChild(section, "usageList").takeFocus()
        keyClick(Qt.Key_Return, Qt.ControlModifier)
        compare(testCase.enterCalls.length, 0, "Ctrl+Return must not drill")
        compare(testCase.locations.length, 1)
        compare(testCase.locations[0], "/home/a/x")
        compare(testCase.navigations, 1)
    }

    function test_e_the_hematita_button_hands_the_current_folder_over() {
        const button = findChild(section, "hematitaButton")
        verify(button !== null)
        mouseClick(button)
        compare(testCase.hematitaCalls.length, 1)
        compare(testCase.hematitaCalls[0], "/home/a")
    }

    function test_f_space_is_left_for_the_host() {
        findChild(section, "usageList").takeFocus()
        keyClick(Qt.Key_Space)
        compare(testCase.enterCalls.length, 0)
        compare(testCase.locations.length, 0)
        compare(testCase.hostKeys.length, 1)
        compare(testCase.hostKeys[0], Qt.Key_Space)
    }

    function test_g_left_and_right_in_the_list_stay_in_the_section() {
        findChild(section, "usageList").takeFocus()
        keyClick(Qt.Key_Left)
        keyClick(Qt.Key_Right)
        compare(testCase.hostKeys.length, 0,
                "Left/Right would step the quick look to another entry")
    }

    function test_h_going_to_a_file_opens_the_folder_that_holds_it() {
        findChild(section, "usageList").takeFocus()
        section.currentId = 11
        keyClick(Qt.Key_Return, Qt.ControlModifier)
        compare(testCase.locations.length, 1)
        compare(testCase.locations[0], "/home/a")
        compare(testCase.navigations, 1)
    }

    function test_i_the_totals_say_scanning_and_failure() {
        const totals = findChild(section, "usageTotals")
        verify(totals !== null)
        usageStub.mode = "scanning"
        verify(totals.text.indexOf("Calculando") === 0)
        usageStub.mode = "failed"
        usageStub.failure = "root"
        compare(totals.text, "No se puede leer esta carpeta")
        usageStub.failure = "not-a-folder"
        compare(totals.text, "No es una carpeta")
    }

    function test_j_enter_on_the_map_drills_through_the_hub() {
        const map = findChild(section, "usageMap")
        verify(map !== null)
        map.contentItem.forceActiveFocus()
        keyClick(Qt.Key_Return)
        compare(testCase.enterCalls.length, 1)
        compare(testCase.enterCalls[0], 10)
    }

    function test_k_a_crumb_jumps_in_one_step() {
        const crumb = findChild(section, "crumb0")
        verify(crumb !== null)
        mouseClick(crumb)
        compare(testCase.upToCalls.length, 1)
        compare(testCase.upToCalls[0], 0)
        compare(testCase.upCalls, 0)
    }

    function test_l_arrows_and_enter_at_the_list_edges_stay_in_the_section() {
        const list = findChild(section, "usageList")
        list.takeFocus()
        keyClick(Qt.Key_Up)
        compare(testCase.hostKeys.length, 0,
                "Up on the first row would step the quick look to another entry")
        usageStub.rowIds = []
        usageStub.revision++
        compare(section.usageRows.length, 0)
        list.takeFocus()
        keyClick(Qt.Key_Enter)
        keyClick(Qt.Key_Return)
        compare(testCase.hostKeys.length, 0,
                "Enter on an empty list would close the quick look and navigate")
        compare(testCase.enterCalls.length, 0)
    }

    function test_m_hematita_during_a_scan_gets_the_scanned_root() {
        usageStub.mode = "scanning"
        usageStub.currentPath = ""
        mouseClick(findChild(section, "hematitaButton"))
        compare(testCase.hematitaCalls.length, 1)
        compare(testCase.hematitaCalls[0], "/home")
    }

    function test_n_a_failed_analysis_shows_no_controls() {
        usageStub.mode = "failed"
        usageStub.failure = "root"
        verify(!findChild(section, "hematitaButton").visible)
        verify(!findChild(section, "goToFolderButton").visible)
    }
}
