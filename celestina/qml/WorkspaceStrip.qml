// The delegate reports its click to this file's root. Bound component
// behaviour is what makes that outer id legal inside the Repeater's component.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: root

    required property bool niriAvailable
    required property string outputName
    // A QVariantList of maps from NiriClient. Each entry guarantees:
    // index, label, output, active, focused, urgent, activeWindowTitle and
    // requestState. `var` is necessary because QML has no typed map-list.
    required property var workspaces
    // A click is a request. The strip asks; whoever owns the provider sends it,
    // and only the next compositor snapshot decides what the pill shows.
    signal focusRequested(string output, int index)
    // A secondary click asks for the panel's context menu at that point, with
    // the workspaces this strip is showing; the host owns every surface this
    // component does not. The menu is a pointer convenience and has no
    // assistive route of its own — Qt Quick exposes no show-menu action — but
    // it adds no action that is only reachable through it: every workspace it
    // offers is the press action of its own pill, and of the compositor's
    // binds.
    signal menuRequested(int globalX, int globalY, var workspaces)

    readonly property var outputWorkspaces: {
        const result = [];
        for (let index = 0; index < workspaces.length; ++index) {
            if (workspaces[index].output === outputName)
                result.push(workspaces[index]);

        }
        return result;
    }
    // The same workspaces folded by the monitor each belongs to. A fold, not a
    // decision: `home`, `groupExpanded` and `groupFocus` are all published by
    // the adapter, which is the process that links the core owning what a home
    // is and which group opens. This file only puts equal homes side by side.
    //
    // A session with every monitor connected produces exactly one group, which
    // renders as the plain row it always was — no capsule, no chrome, nothing
    // to explain.
    readonly property var workspaceGroups: {
        const groups = [];
        const byKey = ({});
        const list = root.outputWorkspaces;
        for (let index = 0; index < list.length; ++index) {
            const workspace = list[index];
            const key = workspace.home !== undefined && workspace.home.length > 0 ? workspace.home : workspace.output;
            let group = byKey[key];
            if (group === undefined) {
                group = {
                    "key": key,
                    "expanded": true,
                    "workspaces": [],
                    "urgent": false,
                    "focusTarget": null
                };
                byKey[key] = group;
                groups.push(group);
            }
            // A group is open when the adapter said its workspaces are. An older
            // helper sends no such field and the host defaults it to true, which
            // is how a producer that predates grouping keeps drawing a flat row.
            group.workspaces.push(workspace);
            group.expanded = group.expanded && workspace.groupExpanded !== false;
            // Urgency must survive the collapse. A capsule that hid a workspace
            // asking for attention would be the fold telling a lie.
            group.urgent = group.urgent || workspace.urgent === true;
            if (group.focusTarget === null || workspace.groupFocus === true)
                group.focusTarget = workspace;

        }
        return groups;
    }
    readonly property bool grouped: workspaceGroups.length > 1
    // Where a step starts from: the newest request still in flight, so a burst
    // of wheel steps advances instead of asking for the same workspace twice,
    // and otherwise what the compositor says is active.
    readonly property int stepOrigin: {
        let pending = -1;
        let active = -1;
        for (let index = 0; index < outputWorkspaces.length; ++index) {
            if (outputWorkspaces[index].requestState === "pending")
                pending = index;

            if (outputWorkspaces[index].active)
                active = index;

        }
        if (pending >= 0)
            return pending;

        return active;
    }
    readonly property string activeWindowTitle: {
        for (let index = 0; index < outputWorkspaces.length; ++index) {
            if (outputWorkspaces[index].active)
                return outputWorkspaces[index].activeWindowTitle;

        }
        return "";
    }

    // One step along this output's workspaces, wrapping at either end. Like a
    // click it is only a request; the pills report what the compositor answers.
    function step(direction) {
        const list = root.outputWorkspaces;
        if (list.length === 0)
            return;

        const from = root.stepOrigin >= 0 ? root.stepOrigin : 0;
        const next = list[(from + direction + list.length) % list.length];
        root.focusRequested(next.output, next.index);
    }

    implicitWidth: workspaceRow.implicitWidth + titleSpacer.width + windowTitle.implicitWidth
    implicitHeight: 28
    Accessible.role: Accessible.List
    Accessible.name: qsTr("Espacios de trabajo de %1").arg(outputName)
    // The panel surface takes no keyboard, so these are the assistive route to
    // the same step the wheel makes; the compositor's own binds remain the
    // keyboard route to switching workspace.
    Accessible.onScrollUpAction: root.step(-1)
    Accessible.onScrollDownAction: root.step(1)

    WheelHandler {
        // A wheel notch is 120 eighths of a degree. Anything smaller is a
        // touchpad's fine scroll and is accumulated rather than dropped.
        property real steps: 0

        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onWheel: (event) => {
            steps += event.angleDelta.y / 120;
            while (steps >= 1) {
                steps -= 1;
                root.step(-1);
            }
            while (steps <= -1) {
                steps += 1;
                root.step(1);
            }
        }
    }

    Text {
        id: unavailableLabel

        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        visible: !root.niriAvailable
        text: qsTr("Niri no disponible")
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
    }

    Row {
        id: workspaceRow

        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceXs
        visible: root.niriAvailable

        Repeater {
            model: root.workspaceGroups

            delegate: Row {
                id: groupRow

                required property var modelData

                spacing: CelestinaTheme.spaceXs

                // An open group is its workspaces. There is no separate
                // "grouped" appearance for them: a strip with one group draws
                // this and only this, which is what it drew before groups
                // existed.
                Repeater {
                    model: groupRow.modelData.expanded ? groupRow.modelData.workspaces : []

                    delegate: WorkspacePill {
                        required property var modelData

                        workspace: modelData
                        onFocusRequested: (output, index) => root.focusRequested(output, index)
                        onMenuRequested: (globalX, globalY) => root.menuRequested(globalX, globalY, root.outputWorkspaces)
                    }

                }

                // The closed shape of the same group. `Row` leaves an invisible
                // child out of its layout, so an open group is not padded by the
                // capsule it is not showing.
                //
                // Deliberately not animated. The strip changes shape when the
                // focus moves between monitors, which is the same instant change
                // every other panel state makes, and an animated reflow here
                // would be motion `CelestinaTheme.reducedMotion` then has to
                // undo. There is nothing to honour because there is nothing
                // moving.
                WorkspaceGroupCapsule {
                    visible: !groupRow.modelData.expanded
                    outputName: groupRow.modelData.key
                    count: groupRow.modelData.workspaces.length
                    urgent: groupRow.modelData.urgent
                    onFocusRequested: {
                        const target = groupRow.modelData.focusTarget;
                        if (target)
                            root.focusRequested(target.output, target.index);

                    }
                    onMenuRequested: (globalX, globalY) => root.menuRequested(globalX, globalY, root.outputWorkspaces)
                }

            }

        }

    }

    Item {
        id: titleSpacer

        anchors.left: workspaceRow.right
        width: windowTitle.visible ? CelestinaTheme.spaceMd : 0
    }

    Text {
        id: windowTitle

        anchors.left: titleSpacer.right
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        visible: root.niriAvailable && root.activeWindowTitle.length > 0
        text: root.activeWindowTitle
        // A window's title is whatever its client set it to, so it is shown as
        // characters rather than guessed at as markup.
        textFormat: Text.PlainText
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        elide: Text.ElideRight
    }

}
