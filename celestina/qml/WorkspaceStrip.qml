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
            model: root.outputWorkspaces

            delegate: Item {
                id: workspaceItem

                required property var modelData
                readonly property string requestState: modelData.requestState
                readonly property string stateDescription: {
                    if (requestState === "pending")
                        return qsTr("cambio solicitado");

                    if (requestState === "failed")
                        return qsTr("el cambio falló");

                    if (requestState === "confirmed")
                        return qsTr("cambio confirmado");

                    return "";
                }

                width: Math.max(24, workspaceLabel.implicitWidth + 12)
                height: 26
                Accessible.role: Accessible.Button
                Accessible.name: {
                    let state = modelData.active ? qsTr("activo") : qsTr("inactivo");
                    if (modelData.urgent)
                        state += ", " + qsTr("requiere atención");

                    return qsTr("Espacio %1, %2").arg(modelData.label).arg(state);
                }
                // The panel surface refuses keyboard focus by design, so this
                // action is the assistive route to it; the compositor's own
                // binds remain the keyboard route to switching workspace.
                Accessible.description: stateDescription
                Accessible.onPressAction: root.focusRequested(modelData.output, modelData.index)

                Rectangle {
                    anchors.fill: parent
                    radius: CelestinaTheme.radiusPill
                    color: {
                        if (workspaceItem.requestState === "failed")
                            return CelestinaTheme.dangerFill;

                        if (workspaceItem.modelData.active)
                            return CelestinaTheme.surfaceSelected;

                        return focusArea.containsMouse ? CelestinaTheme.surfaceHover : CelestinaTheme.clear;
                    }
                    border.width: {
                        if (workspaceItem.requestState.length > 0 || workspaceItem.modelData.focused)
                            return CelestinaTheme.borderHairline;

                        return 0;
                    }
                    border.color: {
                        if (workspaceItem.requestState === "failed")
                            return CelestinaTheme.dangerBorder;

                        if (workspaceItem.requestState === "confirmed")
                            return CelestinaTheme.accentLink;

                        return CelestinaTheme.accentSoftBorder;
                    }
                }

                Text {
                    id: workspaceLabel

                    anchors.centerIn: parent
                    text: workspaceItem.modelData.label
                    textFormat: Text.PlainText
                    color: workspaceItem.modelData.active ? CelestinaTheme.accentLink : CelestinaTheme.textMuted
                    // A request in flight is not a result: the label stays
                    // readable but visibly unsettled until Niri answers.
                    opacity: workspaceItem.requestState === "pending" ? CelestinaTheme.mutedContentOpacity : 1
                    font.family: CelestinaTheme.sansFamily
                    font.features: CelestinaTheme.fontFeaturesTabular
                    font.pixelSize: CelestinaTheme.fontCaption
                    font.weight: workspaceItem.modelData.active ? CelestinaTheme.weightDemiBold : CelestinaTheme.weightRegular
                }

                Rectangle {
                    anchors.top: parent.top
                    anchors.right: parent.right
                    width: 5
                    height: 5
                    radius: CelestinaTheme.radiusPill
                    visible: workspaceItem.modelData.urgent
                    color: CelestinaTheme.danger
                }

                MouseArea {
                    id: focusArea

                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    cursorShape: Qt.PointingHandCursor
                    onClicked: (mouse) => {
                        if (mouse.button === Qt.RightButton) {
                            const anchor = workspaceItem.mapToGlobal(0, workspaceItem.height);
                            root.menuRequested(anchor.x, anchor.y, root.outputWorkspaces);
                            return;
                        }
                        root.focusRequested(workspaceItem.modelData.output, workspaceItem.modelData.index);
                    }
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
