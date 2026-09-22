pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.hematita 1.0

// The column titles, each a sort control. Controls family: they lift, they
// do not paint the accent ramp. The active column shows its direction.
Item {
    id: header

    required property string sortField
    required property bool sortAscending
    // Column widths, shared with the rows so titles sit over values.
    required property var columns

    signal sortRequested(string field)

    implicitHeight: CelestinaTheme.controlHeightSm

    Row {
        anchors.fill: parent

        Repeater {
            model: header.columns

            Item {
                id: cell

                required property var modelData
                readonly property bool active: header.sortField === cell.modelData.field

                width: cell.modelData.width
                height: header.height

                Accessible.role: Accessible.Button
                Accessible.name: cell.modelData.title
                Accessible.onPressAction: header.sortRequested(cell.modelData.field)

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: CelestinaTheme.spaceXs / 2
                    radius: CelestinaTheme.radiusSm
                    color: mouse.pressed ? CelestinaTheme.pressedWash
                         : mouse.containsMouse ? CelestinaTheme.surfaceHover
                         : CelestinaTheme.clear
                }

                Row {
                    anchors.left: parent.left
                    anchors.leftMargin: CelestinaTheme.spaceSm
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: CelestinaTheme.spaceXs
                    layoutDirection: cell.modelData.numeric ? Qt.RightToLeft : Qt.LeftToRight
                    width: cell.width - CelestinaTheme.spaceSm * 2

                    Text {
                        text: cell.modelData.title
                        color: cell.active ? CelestinaTheme.text : CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                        font.weight: CelestinaTheme.weightDemiBold
                    }

                    CelestinaIcon {
                        width: CelestinaTheme.iconSm
                        height: width
                        opacity: cell.active ? 1 : 0
                        name: header.sortAscending ? "view-sort-ascending" : "view-sort-descending"
                        tone: CelestinaIcon.Primary
                    }
                }

                MouseArea {
                    id: mouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: header.sortRequested(cell.modelData.field)
                }
            }
        }
    }
}
