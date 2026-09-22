pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// The column titles, each a sort control. Controls family: they lift, they
// do not paint the accent ramp. The active column shows its direction.
//
// Each cell is a button rather than a rectangle with a `MouseArea`: a column
// that can only be sorted with a pointer is a column half the people who use
// this table cannot sort.
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

            AbstractButton {
                id: cell

                required property var modelData
                readonly property bool active: header.sortField === cell.modelData.field

                width: cell.modelData.width
                height: header.height
                hoverEnabled: true
                focusPolicy: Qt.TabFocus

                Accessible.role: Accessible.Button
                Accessible.name: cell.modelData.title

                onClicked: header.sortRequested(cell.modelData.field)

                background: Item {
                    Rectangle {
                        id: lift

                        anchors.fill: parent
                        anchors.margins: CelestinaTheme.spaceXs / 2
                        radius: CelestinaTheme.radiusSm
                        color: cell.down ? CelestinaTheme.pressedWash
                             : cell.hovered ? CelestinaTheme.surfaceHover
                             : CelestinaTheme.clear
                    }

                    CelestinaFocusRing {
                        target: lift
                        cornerRadius: lift.radius
                        shown: cell.visualFocus
                    }
                }

                // `Row` positions its children's x only, so nothing inside
                // it may anchor; the row itself is what centres.
                contentItem: Item {
                    Row {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.leftMargin: CelestinaTheme.spaceSm
                        anchors.rightMargin: CelestinaTheme.spaceSm
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: CelestinaTheme.spaceXs
                        layoutDirection: cell.modelData.numeric ? Qt.RightToLeft : Qt.LeftToRight

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
                }
            }
        }
    }
}
