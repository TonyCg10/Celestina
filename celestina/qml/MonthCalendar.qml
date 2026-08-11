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

    required property BackdropInk ink

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
    property int dayCellHeight: CelestinaTheme.space2xl
    readonly property var displayLocale: Qt.locale("es_ES")

    readonly property var weekdayNames: [qsTr("lu"), qsTr("ma"), qsTr("mi"),
                                         qsTr("ju"), qsTr("vi"), qsTr("sá"), qsTr("do")]

    spacing: CelestinaTheme.spaceXs

    function step(months) {
        root.shown = new Date(root.shownYear, root.shownMonth + months, 1);
    }

    function monthTitle() {
        const month = root.displayLocale.standaloneMonthName(root.shownMonth);
        const capitalized = month.length > 0
                            ? month.charAt(0).toUpperCase() + month.slice(1)
                            : month;
        return capitalized + " " + root.shownYear;
    }

    Row {
        width: parent.width
        spacing: CelestinaTheme.spaceSm

        BackdropButton {
            ink: root.ink
            text: qsTr("‹")
            helpText: qsTr("El mes anterior")
            onClicked: root.step(-1)
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - CelestinaTheme.space3xl * 2 - parent.spacing * 2
            horizontalAlignment: Text.AlignHCenter
            text: root.monthTitle()
            color: root.ink.primary
            elide: Text.ElideRight
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            font.weight: CelestinaTheme.weightDemiBold
        }

        BackdropButton {
            ink: root.ink
            text: qsTr("›")
            helpText: qsTr("El mes siguiente")
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
                color: root.ink.muted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
            }
        }

        Repeater {
            model: root.leadingBlanks

            delegate: Item {
                width: (grid.width - grid.spacing * 6) / 7
                height: root.dayCellHeight
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
                height: root.dayCellHeight

                Accessible.role: Accessible.StaticText
                Accessible.name: cell.isToday ? qsTr("%1, hoy").arg(cell.day) : String(cell.day)

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
                    color: cell.isToday ? CelestinaTheme.accentInk : root.ink.primary
                    font.family: CelestinaTheme.sansFamily
                    font.features: CelestinaTheme.fontFeaturesTabular
                    font.pixelSize: CelestinaTheme.fontCaption
                }
            }
        }
    }
}
