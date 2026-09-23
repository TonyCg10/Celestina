import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The one transient column: rings at its base, notices above them, errors above
// everything. It grows upward because its own height decides where its top is,
// never because a live positioner changes which end it is anchored to — that is
// what once left the shell's toast cards hanging outside their glass.
TestCase {
    id: testCase
    name: "ActivityStack"
    width: 600
    height: 400
    visible: true
    when: windowShown

    property var dismissed: []

    QtObject {
        id: controllerStub
        property string errorText: ""
        property string opError: ""
        property var noticeRows: []
        property bool opRunning: false
        property var opIds: []
        property var opLabels: []
        property var opIcons: []
        property var opCurrents: []
        property var opDetails: []
        property var opPercents: []
        property var opSteps: []
        property var opPaused: []
        function dismissNotice(id) { testCase.dismissed.push(id) }
        function toggleJobPaused(id) { }
        function cancelJob(id) { }
    }

    Item {
        id: backdropStub
        anchors.fill: parent
    }

    ActivityStack {
        id: stack
        controller: controllerStub
        backdrop: backdropStub
        maxNoticeWidth: 400
        x: 180
        y: 400 - implicitHeight
    }

    function init() {
        testCase.dismissed = []
        controllerStub.errorText = ""
        controllerStub.opError = ""
        controllerStub.noticeRows = []
        controllerStub.opRunning = false
        controllerStub.opIds = []
        controllerStub.opLabels = []
        controllerStub.opIcons = []
        controllerStub.opCurrents = []
        controllerStub.opDetails = []
        controllerStub.opPercents = []
        controllerStub.opSteps = []
        controllerStub.opPaused = []
        mouseMove(testCase, 20, 20)
    }

    function oneNotice(id) {
        controllerStub.noticeRows = [
            id + "\tcircle-stop\tinfo\t0\tPegado cancelado"
        ]
    }

    function test_a_an_empty_stack_takes_no_height() {
        compare(stack.implicitHeight, 0,
                "with nothing running the column is not there at all")
    }

    function test_b_a_settled_notice_gives_the_column_its_height() {
        oneNotice("7")
        tryVerify(function() { return stack.implicitHeight > 0 }, 1000)
    }

    function test_c_errors_sit_above_notices_and_dismiss_through_the_controller() {
        oneNotice("8")
        controllerStub.opError = "No se pudo mover el elemento"
        tryVerify(function() {
            return stack.errorNotice.item !== null && stack.errorNotice.item.shown
        }, 1000)
        // The positioner lays out in the polish phase, so the order is only
        // readable once the event loop has turned.
        tryVerify(function() {
            return stack.errorNotice.y < stack.noticeRepeater.itemAt(0).y
        }, 1000, "the error is above the announcement, not across the rows")
        mouseClick(stack.errorNotice, 10, 10)
        tryVerify(function() {
            return testCase.dismissed.indexOf("op-error") >= 0
        }, 2000, "pressing it asks the controller to clear the error")
    }

    function test_d_the_dock_is_the_base_of_the_column() {
        controllerStub.opRunning = true
        controllerStub.opIds = ["1"]
        controllerStub.opLabels = ["Moviendo…"]
        controllerStub.opIcons = ["arrow-right"]
        controllerStub.opCurrents = [""]
        controllerStub.opDetails = [""]
        controllerStub.opPercents = ["40"]
        controllerStub.opSteps = ["2"]
        controllerStub.opPaused = ["0"]
        oneNotice("9")
        tryVerify(function() { return stack.dock.visible }, 1000)
        tryVerify(function() {
            return stack.dock.y > stack.noticeRepeater.itemAt(0).y
        }, 1000, "the rings are the base; the announcement rests on them")
    }

    function test_da_a_tab_in_a_name_survives_the_cut() {
        controllerStub.noticeRows = [
            "12\tcircle-alert\tdanger\t0\tNo se pudo mover «un\tnombre»"
        ]
        tryVerify(function() {
            return stack.noticeRepeater.itemAt(0) !== null
        }, 1000)
        const entry = stack.noticeRepeater.itemAt(0)
        compare(entry.noticeId, "12")
        compare(entry.iconName, "circle-alert")
        compare(entry.danger, true)
        compare(entry.running, false)
        compare(entry.text, "No se pudo mover «un\tnombre»",
                "the text is the whole remainder after the fourth tab")
    }

    function test_e_a_settled_notice_asks_the_controller_to_drop_it() {
        oneNotice("10")
        tryVerify(function() {
            return stack.noticeRepeater.itemAt(0) !== null
                   && stack.noticeRepeater.itemAt(0).shown
        }, 1000)
        mouseClick(stack.noticeRepeater.itemAt(0), 10, 10)
        tryVerify(function() {
            return testCase.dismissed.indexOf("10") >= 0
        }, 2000, "the notice dismisses by its own id")
    }
}
