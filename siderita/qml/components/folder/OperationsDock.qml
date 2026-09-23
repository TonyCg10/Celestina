pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

    // ── The running write operations ──────────────────────────────────
    // A row of rings, one per job, over the folder rather than across it: a
    // long copy no longer takes a bar's worth of the window for an hour.
    //
    // A ring says only that something is running and which action it is. What
    // it is running *on* is one press away, in a callout that points at the
    // ring it belongs to — so the detail is available without being permanent,
    // and Cancel belongs to the job a person is looking at rather than to
    // whichever one happens to be first.
    //
    // The controller publishes the jobs as parallel lists; nothing is worked
    // out here, and the id that travels back on Cancel is the job's own.
Item {
    id: dock

    property var controller
    // The item the glass samples, and whether there is anything behind it.
    property Item backdrop
    property bool floating: true

    readonly property var jobIds: dock.controller.opIds ?? []
    readonly property int ringSize: 40
    readonly property int gap: CelestinaTheme.spaceSm
    // Which job's callout is open, by id: an id survives the row it was in
    // moving, which an index does not.
    property string openId: ""

    // How much width the column can give the rings. Zero means "no limit",
    // which is what the tests and any consumer that does not measure pass.
    property real availableWidth: 0
    // Beyond three, a row of rings is a row of anonymous circles: the list says
    // which is which. A frame too narrow collapses it at any count.
    readonly property int rowLimit: 3
    readonly property real rowWidth: dock.jobIds.length * dock.ringSize
                                     + Math.max(0, dock.jobIds.length - 1) * dock.gap
                                     + 2 * dock.padding
    readonly property bool collapsed: dock.jobIds.length > dock.rowLimit
                                      || (dock.availableWidth > 0
                                          && dock.rowWidth > dock.availableWidth)
    property bool expanded: false

    readonly property alias countCircle: counted
    // The Repeater, not the Column: the tests read `count` and `itemAt`, which
    // are the Repeater's own.
    readonly property alias jobList: jobRepeater

    function at(list, index) {
        return list !== undefined && index >= 0 && index < list.length ? list[index] : ""
    }
    function indexOfJob(id) {
        for (var i = 0; i < dock.jobIds.length; i++) {
            if (dock.jobIds[i] === id)
                return i
        }
        return -1
    }

    readonly property int padding: CelestinaTheme.spaceSm
    // Sized from the row rather than around it: a `centerIn` here would make the
    // dock's width depend on the row while the row's position depends on the
    // dock's width, and QML answers a cycle like that by laying nothing out —
    // which is how the previous surface ended up drawing its rows on top of one
    // another.
    implicitWidth: (dock.collapsed
                    ? (dock.expanded ? jobRows.implicitWidth : dock.ringSize)
                    : rings.width) + 2 * dock.padding
    implicitHeight: (dock.collapsed && dock.expanded
                     ? jobRows.implicitHeight
                     : dock.ringSize) + 2 * dock.padding
    visible: dock.controller.opRunning

    // Pressing anywhere else closes the callout. The catcher lives in the
    // dock's parent and under the dock, so it covers the folder without
    // covering the rings — a person closing one callout by pressing another
    // ring must still reach that ring.
    //
    // A MouseArea, not a TapHandler: a handler only takes a passive grab, so
    // the press that closed the callout went on to the row under the pointer
    // and selected, opened or dragged it. The area accepts every button and
    // hover, so nothing of that press — or of the cursor — reaches the folder.
    Item {
        id: outsideCatcher
        parent: dock.parent
        anchors.fill: parent
        z: dock.z - 1
        visible: dock.openId.length > 0 || dock.expanded

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
            hoverEnabled: true
            preventStealing: true
            onPressed: {
                dock.openId = ""
                dock.expanded = false
            }
        }
    }

    // Escape closes it too, for a hand that never left the keyboard.
    Shortcut {
        sequence: "Escape"
        enabled: dock.openId.length > 0 || dock.expanded
        onActivated: {
            dock.openId = ""
            dock.expanded = false
        }
    }

    // A job that ends while its callout is open takes the callout with it, and
    // so does a collapse: the callout points at a ring that is no longer drawn.
    onJobIdsChanged: {
        if (dock.openId.length > 0 && dock.indexOfJob(dock.openId) < 0)
            dock.openId = ""
    }
    onCollapsedChanged: {
        if (dock.collapsed)
            dock.openId = ""
        else
            dock.expanded = false
    }

    GlassPill {
        anchors.fill: parent
        backdrop: dock.backdrop
        floating: dock.floating
        fill: CelestinaTheme.controlFill
    }

    Row {
        id: rings
        visible: !dock.collapsed
        x: dock.padding
        y: dock.padding
        spacing: dock.gap

        Repeater {
            model: dock.jobIds.length

            OperationRing {
                id: jobRing
                required property int index
                readonly property string jobId: dock.at(dock.jobIds, jobRing.index)
                objectName: "operationRing-" + jobRing.jobId
                width: dock.ringSize
                height: dock.ringSize
                iconName: dock.at(dock.controller.opIcons, jobRing.index)
                percent: {
                    const raw = parseInt(dock.at(dock.controller.opPercents,
                                                 jobRing.index), 10)
                    return isNaN(raw) ? -1 : raw
                }
                steps: {
                    const raw = parseInt(dock.at(dock.controller.opSteps,
                                                 jobRing.index), 10)
                    return isNaN(raw) ? 0 : raw
                }
                paused: dock.at(dock.controller.opPaused, jobRing.index) === "1"
                active: dock.openId === jobRing.jobId
                Accessible.role: Accessible.Button
                Accessible.name: dock.at(dock.controller.opLabels, jobRing.index)
                onClicked: dock.openId = dock.active(jobRing.jobId) ? "" : jobRing.jobId
            }
        }
    }

    // The collapsed shape: one circle with the count inside and the average of
    // every measurable job on its arc. It is a button, and what it opens is the
    // list below.
    OperationRing {
        id: counted
        objectName: "operationCount"
        visible: dock.collapsed && !dock.expanded
        x: dock.padding
        y: dock.padding
        width: dock.ringSize
        height: dock.ringSize
        readonly property int count: dock.jobIds.length
        // The count replaces the glyph: with four jobs there is no single
        // action to draw, and how many there are is the one thing the circle
        // can say truthfully.
        iconName: ""
        countLabel: counted.count
        percent: {
            let total = 0
            let measured = 0
            for (let index = 0; index < dock.jobIds.length; index++) {
                const raw = parseInt(dock.at(dock.controller.opPercents, index), 10)
                if (!isNaN(raw) && raw >= 0) {
                    total += raw
                    measured++
                }
            }
            return measured > 0 ? Math.round(total / measured) : -1
        }
        steps: {
            let sum = 0
            for (let index = 0; index < dock.jobIds.length; index++) {
                const raw = parseInt(dock.at(dock.controller.opSteps, index), 10)
                sum += isNaN(raw) ? 0 : raw
            }
            return sum
        }
        active: dock.expanded
        Accessible.role: Accessible.Button
        Accessible.name: qsTr("%1 operaciones en curso").arg(counted.count)
        onClicked: dock.expanded = !dock.expanded
    }

    // The expanded shape: the same jobs, named.
    Column {
        id: jobRows
        visible: dock.collapsed && dock.expanded
        x: dock.padding
        y: dock.padding
        spacing: 2

        Repeater {
            id: jobRepeater
            model: dock.jobIds.length

            OperationsListRow {
                id: jobRow
                required property int index
                width: 260
                jobId: dock.at(dock.jobIds, jobRow.index)
                label: dock.at(dock.controller.opLabels, jobRow.index)
                iconName: dock.at(dock.controller.opIcons, jobRow.index)
                percent: {
                    const raw = parseInt(
                        dock.at(dock.controller.opPercents, jobRow.index), 10)
                    return isNaN(raw) ? -1 : raw
                }
                steps: {
                    const raw = parseInt(
                        dock.at(dock.controller.opSteps, jobRow.index), 10)
                    return isNaN(raw) ? 0 : raw
                }
                paused: dock.at(dock.controller.opPaused, jobRow.index) === "1"
                onPauseRequested: id => dock.controller.toggleJobPaused(id)
                onCancelRequested: id => dock.controller.cancelJob(id)
            }
        }
    }

    // Whether this job's callout is the open one, asked as a function so the
    // ring's own handler stays a single expression.
    function active(id) {
        return dock.openId === id
    }

    OperationCallout {
        id: callout
        controller: dock.controller
        backdrop: dock.backdrop
        jobId: dock.openId
        jobIndex: dock.indexOfJob(dock.openId)
        percent: {
            const index = dock.indexOfJob(dock.openId)
            const raw = parseInt(dock.at(dock.controller.opPercents, index), 10)
            return isNaN(raw) ? -1 : raw
        }
        paused: dock.at(dock.controller.opPaused,
                        dock.indexOfJob(dock.openId)) === "1"
        // Points at the ring it belongs to, and sits above the dock.
        pointerX: {
            const index = dock.indexOfJob(dock.openId)
            return index < 0
                   ? dock.width / 2
                   : rings.x + index * (dock.ringSize + dock.gap) + dock.ringSize / 2
        }
        // Centred on its ring, but never past the dock's right edge — the dock
        // rests against the right of the window, so a callout wider than the
        // dock grows leftwards instead of off the screen.
        x: Math.min(dock.width - callout.width,
                    callout.pointerX - callout.width / 2)
        y: -callout.height - CelestinaTheme.spaceXs
        onDismissed: dock.openId = ""
    }
}
