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
    signal menuRequested(int globalX, int globalY)

    readonly property string requestState: workspace.requestState
    readonly property string stateDescription: {
        if (requestState === "pending")
            return qsTr("cambio solicitado");

        if (requestState === "failed")
            return qsTr("el cambio falló");

        if (requestState === "confirmed")
            return qsTr("cambio confirmado");

        return "";
    }

    implicitWidth: Math.max(24, workspaceLabel.implicitWidth + 12)
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
        anchors.fill: parent
        radius: CelestinaTheme.radiusPill
        color: {
            if (pill.requestState === "failed")
                return CelestinaTheme.dangerFill;

            if (pill.workspace.active)
                return CelestinaTheme.surfaceSelected;

            return focusArea.containsMouse ? CelestinaTheme.surfaceHover : CelestinaTheme.clear;
        }
        border.width: {
            if (pill.requestState.length > 0 || pill.workspace.focused)
                return CelestinaTheme.borderHairline;

            return 0;
        }
        border.color: {
            if (pill.requestState === "failed")
                return CelestinaTheme.dangerBorder;

            if (pill.requestState === "confirmed")
                return CelestinaTheme.accentLink;

            return CelestinaTheme.accentSoftBorder;
        }
    }

    Text {
        id: workspaceLabel

        anchors.centerIn: parent
        text: pill.workspace.label
        textFormat: Text.PlainText
        color: pill.workspace.active ? CelestinaTheme.accentLink : CelestinaTheme.textMuted
        // A request in flight is not a result: the label stays readable but
        // visibly unsettled until Niri answers.
        opacity: pill.requestState === "pending" ? CelestinaTheme.mutedContentOpacity : 1
        font.family: CelestinaTheme.sansFamily
        font.features: CelestinaTheme.fontFeaturesTabular
        font.pixelSize: CelestinaTheme.fontCaption
        font.weight: pill.workspace.active ? CelestinaTheme.weightDemiBold : CelestinaTheme.weightRegular
    }

    Rectangle {
        anchors.top: parent.top
        anchors.right: parent.right
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
                pill.menuRequested(anchor.x, anchor.y);
                return;
            }
            pill.focusRequested(pill.workspace.output, pill.workspace.index);
        }
    }

}
