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
    // What a workspace or a whole monitor group holds, asked for by the right
    // button. The strip supplies the workspaces because it is the only thing
    // that knows which ones a capsule folded; the host owns the surface that
    // answers.
    signal mapRequested(int globalX, int globalY, var workspaces)

    // A closed monitor capsule can be opened for direct workspace selection.
    // The compositor's next focused-group answer clears that temporary choice.
    property string chosenGroup: ""

    readonly property string focusedGroup: {
        const list = root.outputWorkspaces;
        for (let index = 0; index < list.length; ++index) {
            if (list[index].active === true)
                return list[index].home !== undefined && list[index].home.length > 0
                       ? list[index].home : list[index].output;

        }
        return "";
    }
    onFocusedGroupChanged: root.chosenGroup = ""

    readonly property var outputWorkspaces: {
        const result = [];
        for (let index = 0; index < workspaces.length; ++index) {
            if (workspaces[index].output === outputName)
                result.push(workspaces[index]);

        }
        return result;
    }
    // Fold workspaces by their monitor home. Exactly one group stays expanded;
    // the others retain the compact monitor capsule the panel used before the
    // visual redesign. Labels leave both shapes, but their positional grouping
    // and interaction do not.
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
            group.workspaces.push(workspace);
            group.expanded = group.expanded && workspace.groupExpanded !== false;
            if (root.chosenGroup.length > 0)
                group.expanded = group.key === root.chosenGroup;

            group.urgent = group.urgent || workspace.urgent === true;
            if (group.focusTarget === null || workspace.groupFocus === true)
                group.focusTarget = workspace;
        }

        // Niri's nested backend adds one empty spare workspace on its synthetic
        // `winit` output. It is neither a configured workspace nor a useful
        // monitor group, and the same rule removes any equivalent unoccupied,
        // inactive singleton spare without special-casing that backend name.
        return groups.filter((group) => {
            if (groups.length <= 1 || group.key !== root.outputName
                || group.workspaces.length !== 1) {
                return true;
            }
            const workspace = group.workspaces[0];
            return workspace.active === true || workspace.focused === true
                   || workspace.urgent === true
                   || (workspace.activeWindowTitle !== undefined
                       && workspace.activeWindowTitle.length > 0);
        });
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

    implicitWidth: workspaceRow.implicitWidth
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
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontTitle
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

                Repeater {
                    model: groupRow.modelData.expanded ? groupRow.modelData.workspaces : []

                    delegate: WorkspacePill {
                        required property var modelData

                        workspace: modelData
                        onFocusRequested: (output, index) => root.focusRequested(output, index)
                        onMapRequested: (globalX, globalY) => root.mapRequested(
                            globalX, globalY, [modelData])
                    }

                }

                WorkspaceGroupCapsule {
                    visible: !groupRow.modelData.expanded
                    outputName: groupRow.modelData.key
                    count: groupRow.modelData.workspaces.length
                    urgent: groupRow.modelData.urgent
                    onExpandRequested: root.chosenGroup = groupRow.modelData.key
                    onMapRequested: (globalX, globalY) => root.mapRequested(
                        globalX, globalY, groupRow.modelData.workspaces)
                }

            }

        }

    }

}
