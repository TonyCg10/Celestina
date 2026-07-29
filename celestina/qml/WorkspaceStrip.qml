import CelestinaStyle
import QtQuick

Item {
    id: root

    required property bool niriAvailable
    required property string outputName
    // A QVariantList of maps from NiriClient. Each entry guarantees:
    // index, label, output, active, focused, urgent and activeWindowTitle.
    // `var` is necessary because QML has no typed map-list.
    required property var workspaces
    readonly property var outputWorkspaces: {
        const result = [];
        for (let index = 0; index < workspaces.length; ++index) {
            if (workspaces[index].output === outputName)
                result.push(workspaces[index]);

        }
        return result;
    }
    readonly property string activeWindowTitle: {
        for (let index = 0; index < outputWorkspaces.length; ++index) {
            if (outputWorkspaces[index].active)
                return outputWorkspaces[index].activeWindowTitle;

        }
        return "";
    }

    implicitHeight: 28
    Accessible.role: Accessible.List
    Accessible.name: qsTr("Espacios de trabajo de %1").arg(outputName)

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

                width: Math.max(24, workspaceLabel.implicitWidth + 12)
                height: 26
                Accessible.role: Accessible.ListItem
                Accessible.name: {
                    let state = modelData.active ? qsTr("activo") : qsTr("inactivo");
                    if (modelData.urgent)
                        state += ", " + qsTr("requiere atención");

                    return qsTr("Espacio %1, %2").arg(modelData.label).arg(state);
                }

                Rectangle {
                    anchors.fill: parent
                    radius: CelestinaTheme.radiusPill
                    color: workspaceItem.modelData.active ? CelestinaTheme.surfaceSelected : CelestinaTheme.clear
                    border.width: workspaceItem.modelData.focused ? CelestinaTheme.borderHairline : 0
                    border.color: CelestinaTheme.accentSoftBorder
                }

                Text {
                    id: workspaceLabel

                    anchors.centerIn: parent
                    text: workspaceItem.modelData.label
                    color: workspaceItem.modelData.active ? CelestinaTheme.accentLink : CelestinaTheme.textMuted
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

            }

        }

    }

    Text {
        anchors.left: workspaceRow.right
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        visible: root.niriAvailable && root.activeWindowTitle.length > 0
        text: root.activeWindowTitle
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        elide: Text.ElideRight
    }

}
