// One workspace, as the strip draws it when its monitor's group is open.
//
// Extracted from `WorkspaceStrip.qml` when the strip gained a second thing to
// draw: a pill and a collapsed group's capsule are different shapes with
// different states, and the strip's job became choosing between them rather
// than being one of them. It owns a workspace's appearance and its press
// action, and nothing about grouping.
import CelestinaStyle
import QtQuick

Item {
    id: pill

    // One entry of the host's workspace list. Guarantees index, label, output,
    // active, focused, urgent and requestState. `var` is necessary because QML
    // has no typed map-list.
    required property var workspace
    // A click is a request. The pill asks; whoever owns the provider sends it,
    // and only the next compositor snapshot decides what this shows.
    signal focusRequested(string output, int index)
    // A secondary click asks the strip for the panel's context menu here.
    // The right button opens this workspace's own map — what it holds, without
    // going to it. It replaces the panel's list-of-workspaces menu that used to
    // live on this gesture: that menu offered the workspaces this strip already
    // shows, and the map answers the question the strip cannot.
    signal mapRequested(int globalX, int globalY)

    readonly property string requestState: workspace.requestState
    readonly property bool occupied: workspace.activeWindowTitle !== undefined
                                     && workspace.activeWindowTitle.length > 0
    readonly property string stateDescription: {
        if (requestState === "pending")
            return qsTr("cambio solicitado");

        if (requestState === "failed")
            return qsTr("el cambio falló");

        if (requestState === "confirmed")
            return qsTr("cambio confirmado");

        return "";
    }

    implicitWidth: 18
    implicitHeight: 26
    Accessible.role: Accessible.Button
    Accessible.name: {
        let state = pill.workspace.active ? qsTr("activo") : qsTr("inactivo");
        if (pill.workspace.urgent)
            state += ", " + qsTr("requiere atención");

        return qsTr("Espacio %1, %2").arg(pill.workspace.label).arg(state);
    }
    // The panel surface refuses keyboard focus by design, so this action is the
    // assistive route to it; the compositor's own binds remain the keyboard
    // route to switching workspace.
    Accessible.description: stateDescription
    Accessible.onPressAction: pill.focusRequested(pill.workspace.output, pill.workspace.index)

    Rectangle {
        id: stateMark
        objectName: "celestina-workspace-state"

        anchors.centerIn: parent
        width: pill.workspace.focused ? 12 : 10
        height: width
        radius: width / 2
        color: {
            if (pill.requestState === "failed" || pill.workspace.urgent)
                return CelestinaTheme.danger;

            if (pill.workspace.focused)
                return CelestinaTheme.accentLink;

            if (pill.occupied)
                return CelestinaTheme.text;

            return CelestinaTheme.textMuted;
        }
        opacity: pill.requestState === "pending"
                 ? CelestinaTheme.mutedContentOpacity
                 : focusArea.containsMouse && !pill.workspace.focused ? 0.82 : 1
        border.width: pill.workspace.focused ? CelestinaTheme.borderHairline : 0
        border.color: {
            if (pill.requestState === "failed")
                return CelestinaTheme.dangerBorder;

            if (pill.requestState === "confirmed")
                return CelestinaTheme.text;

            return CelestinaTheme.accentSoftBorder;
        }
    }

    Rectangle {
        anchors.top: stateMark.top
        anchors.right: stateMark.right
        width: 5
        height: 5
        radius: CelestinaTheme.radiusPill
        visible: pill.workspace.urgent
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
                const anchor = pill.mapToGlobal(0, pill.height);
                pill.mapRequested(anchor.x, anchor.y);
                return;
            }
            pill.focusRequested(pill.workspace.output, pill.workspace.index);
        }
    }

}
