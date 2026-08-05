// The month, drawn from arithmetic rather than fetched from anywhere.
//
// A calendar needs no provider, no service and no permission: the shape of a
// month follows from a rule that has not changed since 1582. The only thing
// this needs from outside is which day it is, and the machine already knows.
//
// Weeks start on Monday, as they do where this session is used.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Column {
    id: root

    // Which month is shown. Today's, until somebody steps away from it.
    property date shown: new Date()
    readonly property date today: new Date()

    readonly property int shownYear: shown.getFullYear()
    readonly property int shownMonth: shown.getMonth()
    readonly property int daysInMonth: new Date(root.shownYear, root.shownMonth + 1, 0).getDate()
    // JavaScript counts Sunday as 0; this grid counts weeks from Monday.
    readonly property int leadingBlanks: (new Date(root.shownYear, root.shownMonth, 1).getDay() + 6) % 7
    readonly property bool showingThisMonth: root.shownYear === root.today.getFullYear()
                                             && root.shownMonth === root.today.getMonth()

    readonly property var weekdayNames: [qsTr("Mo"), qsTr("Tu"), qsTr("We"),
                                         qsTr("Th"), qsTr("Fr"), qsTr("Sa"), qsTr("Su")]

    spacing: CelestinaTheme.spaceXs

    function step(months) {
        root.shown = new Date(root.shownYear, root.shownMonth + months, 1);
    }

    Row {
        width: parent.width
        spacing: CelestinaTheme.spaceSm

        CelestinaButton {
            text: qsTr("‹")
            helpText: qsTr("The month before")
            onClicked: root.step(-1)
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - CelestinaTheme.space3xl * 2 - parent.spacing * 2
            horizontalAlignment: Text.AlignHCenter
            text: Qt.locale().standaloneMonthName(root.shownMonth) + " " + root.shownYear
            color: CelestinaTheme.text
            elide: Text.ElideRight
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            font.weight: CelestinaTheme.weightDemiBold
        }

        CelestinaButton {
            text: qsTr("›")
            helpText: qsTr("The month after")
            onClicked: root.step(1)
        }
    }

    Grid {
        id: grid

        width: parent.width
        columns: 7
        spacing: 2

        Repeater {
            model: root.weekdayNames

            delegate: Text {
                required property var modelData

                width: (grid.width - grid.spacing * 6) / 7
                horizontalAlignment: Text.AlignHCenter
                text: modelData
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
            }
        }

        Repeater {
            model: root.leadingBlanks

            delegate: Item {
                width: (grid.width - grid.spacing * 6) / 7
                height: CelestinaTheme.space2xl
            }
        }

        Repeater {
            model: root.daysInMonth

            delegate: Item {
                id: cell

                required property int index
                readonly property int day: cell.index + 1
                readonly property bool isToday: root.showingThisMonth
                                                && cell.day === root.today.getDate()

                width: (grid.width - grid.spacing * 6) / 7
                height: CelestinaTheme.space2xl

                Accessible.role: Accessible.StaticText
                Accessible.name: cell.isToday ? qsTr("%1, today").arg(cell.day) : String(cell.day)

                Rectangle {
                    anchors.centerIn: parent
                    width: Math.min(parent.width, parent.height)
                    height: width
                    radius: width / 2
                    visible: cell.isToday
                    color: CelestinaTheme.accent
                }

                Text {
                    anchors.centerIn: parent
                    text: cell.day
                    color: cell.isToday ? CelestinaTheme.accentInk : CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.features: CelestinaTheme.fontFeaturesTabular
                    font.pixelSize: CelestinaTheme.fontCaption
                }
            }
        }
    }
}
