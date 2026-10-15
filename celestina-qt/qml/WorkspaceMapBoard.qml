// One workspace: its name, and every window on it as a readable row.
//
// This was drawn as the layout itself — columns side by side at their real
// proportions — and that was wrong for one plain reason: a column of a
// three-column workspace is a third of a narrow card, and every window name in
// it elided to nothing. A map whose labels cannot be read has stopped answering
// the question it exists for.
//
// So the arrangement is told in the order rather than in the geometry. Rows come
// out of `celestina_shell_core::workspace_map` already folded — columns left to
// right, rows down inside a column, floating windows last — and this file lists
// them in that order at full width. Nothing here decides that order.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: root

    // One workspace's published row.
    required property var workspace
    required property BackdropInk ink
    // Whether this board is the workspace the session is on.
    property bool current: false
    // Which row the keyboard is on, across the whole card. A key rather than an
    // index so a board never has to know how the card counts its rows.
    property string currentKey: ""
    // Going to the workspace itself, and going to one window on it.
    signal activated()
    signal windowActivated(string windowId)

    readonly property var map: root.workspace.map !== undefined
                               ? root.workspace.map : null
    readonly property int hidden: root.map !== null && root.map.hidden !== undefined
                                  ? root.map.hidden : 0
    readonly property string label: root.workspace.label !== undefined
                                    ? root.workspace.label : ""

    // Every window, in the reading order the helper folded them into. The
    // columns are flattened here rather than drawn as columns; which column a
    // window came from is kept so a later revision can group by it without the
    // adapter having to publish anything new.
    readonly property var rows: {
        const result = [];
        if (root.map === null)
            return result;

        const columns = root.map.columns !== undefined ? root.map.columns : [];
        for (let index = 0; index < columns.length; ++index) {
            const windows = columns[index].windows;
            for (let row = 0; row < windows.length; ++row)
                result.push(windows[row]);

        }
        const floating = root.map.floating !== undefined ? root.map.floating : [];
        for (let index = 0; index < floating.length; ++index)
            result.push(floating[index]);

        return result;
    }
    readonly property bool holdsNothing: root.rows.length === 0

    implicitWidth: 280
    implicitHeight: content.implicitHeight
    Accessible.role: Accessible.Grouping
    Accessible.name: root.holdsNothing
                     ? qsTr("Espacio %1, vacío").arg(root.label)
                     : qsTr("Espacio %1, %n ventana(s)", "", root.rows.length).arg(root.label)

    Column {
        id: content

        width: parent.width
        spacing: CelestinaTheme.spaceXs

        // The workspace's own row. Going to a workspace and going to a window on
        // it are different requests, so they are different targets rather than
        // one control that guesses which was meant.
        Item {
            width: parent.width
            height: CelestinaTheme.controlHeightXs

            readonly property bool keyboardHere: root.currentKey
                                                 === "workspace:" + root.workspace.index

            Rectangle {
                anchors.fill: parent
                radius: CelestinaTheme.radiusSm
                border.width: parent.keyboardHere ? CelestinaTheme.borderFocus : 0
                border.color: root.ink.focus
                color: headerPointer.pressed
                       ? root.ink.selectedFill
                       : (headerPointer.containsMouse ? root.ink.hoverFill
                                                      : CelestinaTheme.clear)

                Behavior on color {
                    enabled: !CelestinaTheme.reducedMotion

                    ColorAnimation {
                        duration: CelestinaTheme.motionFast
                        easing.type: CelestinaTheme.easeStandard
                    }

                }

            }

            Row {
                anchors.fill: parent
                anchors.leftMargin: CelestinaTheme.spaceSm
                anchors.rightMargin: CelestinaTheme.spaceSm
                spacing: CelestinaTheme.spaceSm

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.label
                    color: root.current ? root.ink.accent : root.ink.primary
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowTitle
                    font.weight: CelestinaTheme.weightDemiBold
                    font.features: CelestinaTheme.fontFeaturesTabular
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.holdsNothing
                          ? qsTr("vacío")
                          : qsTr("%n ventana(s)", "", root.rows.length)
                    color: root.ink.faint
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontMini
                }

            }

            MouseArea {
                id: headerPointer

                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.activated()
            }

            Accessible.role: Accessible.Button
            Accessible.name: qsTr("Ir al espacio %1").arg(root.label)
            Accessible.onPressAction: root.activated()
        }

        Repeater {
            model: root.rows

            delegate: WorkspaceMapTile {
                required property var modelData

                width: content.width
                window: modelData
                ink: root.ink
                current: root.currentKey === "window:" + modelData.id
                onActivated: root.windowActivated(
                    modelData.id !== undefined ? modelData.id : "")
            }

        }

        // A workspace with nothing in it says so. An empty space with no words is
        // indistinguishable from one that failed to draw.
        Text {
            width: parent.width
            visible: root.holdsNothing
            leftPadding: CelestinaTheme.spaceSm
            text: qsTr("No hay ninguna ventana aquí.")
            color: root.ink.faint
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
            wrapMode: Text.WordWrap
        }

        // A board that is not showing everything says how much it is not
        // showing. Four of nine listed silently is the map lying about the one
        // thing it exists to answer.
        Text {
            width: parent.width
            visible: root.hidden > 0
            leftPadding: CelestinaTheme.spaceSm
            text: qsTr("y %n más sin mostrar", "", root.hidden)
            color: root.ink.faint
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontMini
            elide: Text.ElideRight
        }

    }

}
