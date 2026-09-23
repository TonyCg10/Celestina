pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.hematita 1.0

// Where the person is: a button back to the locations, then one ghost button
// per folder from the location's root to the current one. A crumb names its
// index; the hub resolves it against the path it owns, so no path text ever
// comes back from here.
Row {
    id: crumbs

    // Display names, root first; the last is the current folder.
    required property var names

    // `-1` is the locations; `0..` a crumb.
    signal chosen(int index)

    spacing: CelestinaTheme.spaceXs

    // Qt has no navigation-landmark role; a tool bar is the nearest one a
    // screen reader announces as a group of buttons.
    Accessible.role: Accessible.ToolBar
    Accessible.name: qsTr("Ruta")

    CelestinaIconButton {
        anchors.verticalCenter: parent.verticalCenter
        iconName: "hard-drive"
        helpText: qsTr("Ubicaciones")
        role: CelestinaButton.Ghost
        onClicked: crumbs.chosen(-1)
    }

    Repeater {
        model: crumbs.names.length

        delegate: Row {
            id: crumb

            required property int index

            anchors.verticalCenter: parent.verticalCenter
            spacing: CelestinaTheme.spaceXs

            CelestinaIcon {
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.iconSm
                height: width
                name: "chevron-right"
                tone: CelestinaIcon.Secondary
            }

            CelestinaButton {
                anchors.verticalCenter: parent.verticalCenter
                role: CelestinaButton.Ghost
                text: crumb.index < crumbs.names.length ? crumbs.names[crumb.index] : ""
                helpText: crumb.index < crumbs.names.length ? crumbs.names[crumb.index] : ""
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                font.weight: crumb.index === crumbs.names.length - 1
                             ? CelestinaTheme.weightDemiBold : Font.Normal
                onClicked: crumbs.chosen(crumb.index)
            }
        }
    }
}
