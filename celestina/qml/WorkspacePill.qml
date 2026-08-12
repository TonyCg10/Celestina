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
    required property BackdropInk ink
    // A click is a request. The pill asks; whoever owns the provider sends it,
    // and only the next compositor snapshot decides what this shows.
    signal focusRequested(string output, int index)
    // A secondary click asks the strip for the panel's context menu here.
    // The right button opens this workspace's own map — what it holds, without
    // going to it. It replaces the panel's list-of-workspaces menu that used to
    // live on this gesture: that menu offered the workspaces this strip already
    // shows, and the map answers the question the strip cannot.
    signal mapRequested(rect openerRect, rect attachmentAnchorRect)

    // The semantic attachment-source contract the panel lease resolves: the
    // pill is the placement opener and its state dot is the exact glyph the
    // contextual droplet's mouth follows.
    readonly property bool isPanelAttachmentSource: true
    readonly property Item attachmentAnchor: stateMark
    // Set only by the tokened attachment lease that owns the currently mapped
    // contextual surface; the dot keeps its hover emphasis until that exact
    // surface retires.
    property bool menuOpen: false

    function globalRect(item) {
        const topLeft = item.mapToGlobal(0, 0);
        const bottomRight = item.mapToGlobal(item.width, item.height);
        return Qt.rect(Math.min(topLeft.x, bottomRight.x),
                       Math.min(topLeft.y, bottomRight.y),
                       Math.abs(bottomRight.x - topLeft.x),
                       Math.abs(bottomRight.y - topLeft.y));
    }

    function attachmentAnchorGlobalRectNow() {
        return pill.globalRect(pill.attachmentAnchor);
    }

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
        scale: focusArea.pressed ? 0.82 : 1
        radius: width / 2
        color: {
            if (pill.requestState === "failed" || pill.workspace.urgent)
                return pill.ink.danger;

            if (pill.workspace.focused)
                return pill.ink.accent;

            if (pill.occupied)
                return pill.ink.primary;

            return pill.ink.muted;
        }
        opacity: pill.requestState === "pending"
                 ? CelestinaTheme.mutedContentOpacity
                 : focusArea.pressed ? CelestinaTheme.disabledContentOpacity
                 : (focusArea.containsMouse || pill.menuOpen)
                   && !pill.workspace.focused ? 0.82 : 1
        border.width: pill.workspace.focused ? CelestinaTheme.borderHairline : 0
        border.color: {
            if (pill.requestState === "failed")
                return pill.ink.danger;

            if (pill.requestState === "confirmed")
                return pill.ink.primary;

            return pill.ink.focus;
        }

        Behavior on scale {
            NumberAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }
    }

    Rectangle {
        anchors.top: stateMark.top
        anchors.right: stateMark.right
        width: 5
        height: 5
        radius: CelestinaTheme.radiusPill
        visible: pill.workspace.urgent
        color: pill.ink.danger
    }

    MouseArea {
        id: focusArea
        objectName: "celestina-workspace-pointer"

        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        onClicked: (mouse) => {
            if (mouse.button === Qt.RightButton) {
                pill.mapRequested(pill.globalRect(pill),
                                  pill.attachmentAnchorGlobalRectNow());
                return;
            }
            pill.focusRequested(pill.workspace.output, pill.workspace.index);
        }
    }

}
