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
    implicitWidth: rings.width + 2 * dock.padding
    implicitHeight: dock.ringSize + 2 * dock.padding
    visible: dock.controller.opRunning

    // Pressing anywhere else closes the callout. The catcher lives in the
    // dock's parent and under the dock, so it covers the folder without
    // covering the rings — a person closing one callout by pressing another
    // ring must still reach that ring.
    Item {
        id: outsideCatcher
        parent: dock.parent
        anchors.fill: parent
        z: dock.z - 1
        visible: dock.openId.length > 0

        TapHandler {
            onTapped: dock.openId = ""
        }
    }

    // Escape closes it too, for a hand that never left the keyboard.
    Shortcut {
        sequence: "Escape"
        enabled: dock.openId.length > 0
        onActivated: dock.openId = ""
    }

    // A job that ends while its callout is open takes the callout with it.
    onJobIdsChanged: {
        if (dock.openId.length > 0 && dock.indexOfJob(dock.openId) < 0)
            dock.openId = ""
    }

    GlassPill {
        anchors.fill: parent
        backdrop: dock.backdrop
        floating: dock.floating
        fill: CelestinaTheme.controlFill
    }

    Row {
        id: rings
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
                active: dock.openId === jobRing.jobId
                Accessible.role: Accessible.Button
                Accessible.name: dock.at(dock.controller.opLabels, jobRing.index)
                onClicked: dock.openId = dock.active(jobRing.jobId) ? "" : jobRing.jobId
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
