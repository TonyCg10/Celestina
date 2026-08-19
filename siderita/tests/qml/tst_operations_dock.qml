import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The operations dock: one ring per running write, the action's own icon in
// each, and a callout that points at the ring it belongs to. A ring whose end
// cannot be predicted turns instead of filling. Cancel names the job's id, not
// the ring's position.
TestCase {
    id: testCase
    name: "OperationsDock"
    width: 600
    height: 400
    visible: true
    when: windowShown

    property var cancelled: []

    QtObject {
        id: controllerStub
        property bool opRunning: false
        property var opIds: []
        property var opLabels: []
        property var opIcons: []
        property var opCurrents: []
        property var opDetails: []
        property var opPercents: []
        property var opSteps: []

        function cancelJob(id) { testCase.cancelled.push(id) }
    }

    Item {
        id: backdropStub
        width: 600
        height: 400
    }

    OperationsDock {
        id: dock
        controller: controllerStub
        backdrop: backdropStub
        y: 200
    }

    function init() {
        testCase.cancelled = []
        controllerStub.opIds = []
        controllerStub.opLabels = []
        controllerStub.opIcons = []
        controllerStub.opCurrents = []
        controllerStub.opDetails = []
        controllerStub.opPercents = []
        controllerStub.opSteps = []
        controllerStub.opRunning = false
        dock.openId = ""
    }

    function load(jobs) {
        var ids = [], labels = [], icons = [], currents = [], details = [], percents = [], steps = []
        for (var i = 0; i < jobs.length; i++) {
            ids.push(jobs[i].id)
            labels.push(jobs[i].label)
            icons.push(jobs[i].icon)
            currents.push(jobs[i].current)
            details.push(jobs[i].detail)
            percents.push(jobs[i].percent)
            steps.push(jobs[i].steps !== undefined ? jobs[i].steps : "0")
        }
        controllerStub.opIds = ids
        controllerStub.opLabels = labels
        controllerStub.opIcons = icons
        controllerStub.opCurrents = currents
        controllerStub.opDetails = details
        controllerStub.opPercents = percents
        controllerStub.opSteps = steps
        controllerStub.opRunning = jobs.length > 0
    }

    function copying(id) {
        return { id: id, label: "Copiando…", icon: "copy",
                 current: "uno" + id + ".txt", detail: "10 MB copiados",
                 percent: "35" }
    }
    function extracting(id) {
        return { id: id, label: "Extrayendo…", icon: "archive-extract",
                 current: "juego.rar", detail: "11,7 GiB", 
                 percent: "-1" }
    }

    function test_a_nothing_running_shows_no_dock() {
        compare(dock.visible, false)
        load([copying("7")])
        compare(dock.visible, true)
    }

    function test_b_one_ring_per_job_wearing_its_own_action() {
        load([copying("7"), extracting("8")])
        const copy = findChild(dock, "operationRing-7")
        const extract = findChild(dock, "operationRing-8")
        verify(copy !== null && extract !== null)
        compare(copy.iconName, "copy")
        compare(extract.iconName, "archive-extract")
        // Side by side, not stacked: two rings occupy two widths.
        waitForRendering(dock)
        verify(extract.mapToItem(dock, 0, 0).x > copy.mapToItem(dock, 0, 0).x)
    }

    function test_c_an_unknowable_end_turns_instead_of_filling() {
        load([copying("7"), extracting("8")])
        const filling = findChild(dock, "operationRing-7")
        const turning = findChild(dock, "operationRing-8")
        compare(filling.indeterminate, false)
        compare(turning.indeterminate, true)

        // Turning is driven by the reports themselves, not by an animation: one
        // more report, one step further round. A ring that filled instead would
        // keep its start angle at twelve o'clock forever.
        const before = turning.targetStart
        load([copying("7"), { id: "8", label: "Extrayendo…", icon: "archive-extract",
                              current: "juego.rar", detail: "12 GiB",
                              percent: "-1", steps: "4" }])
        verify(Math.abs(turning.targetStart - before) > 1)
        // And the one that fills never leaves twelve o'clock.
        compare(filling.targetStart, -90)
    }

    // The pointer must sit under the ring that opened the callout — checked on
    // the drawn rectangle, in the dock's own coordinates, because a property
    // holding the right number proves nothing about where the shape landed.
    function test_d_pressing_a_ring_opens_the_callout_pointing_at_it() {
        load([copying("7"), extracting("8")])
        waitForRendering(dock)
        const second = findChild(dock, "operationRing-8")
        second.clicked()
        compare(dock.openId, "8")
        waitForRendering(dock)

        const pointer = findChild(dock, "calloutPointer")
        verify(pointer !== null)
        const pointerCentre = pointer.parent.x + pointer.x + pointer.width / 2
        const ringCentre = second.mapToItem(dock, second.width / 2, 0).x
        verify(Math.abs(pointerCentre - ringCentre) < 3)

        // Pressing the same ring again closes it.
        second.clicked()
        compare(dock.openId, "")
    }

    // The same, for the case the author hit: one job, and a callout far wider
    // than the dock it hangs from.
    function test_da_a_lone_ring_is_still_pointed_at() {
        load([extracting("8")])
        waitForRendering(dock)
        const only = findChild(dock, "operationRing-8")
        only.clicked()
        waitForRendering(dock)

        const pointer = findChild(dock, "calloutPointer")
        verify(pointer !== null)
        const pointerCentre = pointer.parent.x + pointer.x + pointer.width / 2
        const ringCentre = only.mapToItem(dock, only.width / 2, 0).x
        verify(Math.abs(pointerCentre - ringCentre) < 3)
    }

    function test_db_pressing_outside_closes_the_callout() {
        load([extracting("8")])
        waitForRendering(dock)
        findChild(dock, "operationRing-8").clicked()
        compare(dock.openId, "8")
        // Away from the dock entirely: the catcher covers the folder behind it.
        mouseClick(testCase, 40, 40)
        compare(dock.openId, "")
    }

    function test_e_the_callouts_cancel_names_that_job() {
        load([copying("7"), extracting("8")])
        findChild(dock, "operationRing-8").clicked()
        const cancel = findChild(dock, "calloutCancel")
        verify(cancel !== null)
        cancel.clicked()
        compare(testCase.cancelled.length, 1)
        compare(testCase.cancelled[0], 8)
        // Cancelling closes the callout it belonged to.
        compare(dock.openId, "")
    }

    function test_f_a_job_that_ends_takes_its_callout_with_it() {
        load([copying("7"), extracting("8")])
        findChild(dock, "operationRing-8").clicked()
        compare(dock.openId, "8")
        load([copying("7")])
        compare(dock.openId, "")
    }
}
