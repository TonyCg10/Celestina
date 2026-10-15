// The month, hanging from the clock that names the day.
//
// A calendar is spatial content, not a list of interchangeable rows, so like
// the wallpaper gallery this is a card rather than a `SoftMenu`. It reads
// nothing from any provider and asks for nothing: the `MonthCalendar` it
// carries is the Control Centre's, with its own month navigation, and closing
// this surface forgets which month was being looked at — reopening always
// starts at today, because the clock is what opened it.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

SoftCard {
    id: root

    readonly property var uiLocale: Qt.locale("es_ES")
    readonly property string todayLine: {
        const now = new Date();
        const line = now.toLocaleString(root.uiLocale, "dddd dd 'de' MMMM 'de' yyyy");
        return line.length > 0
               ? line.charAt(0).toUpperCase() + line.slice(1) : line;
    }

    title: qsTr("Calendario")
    subtitle: root.todayLine
    iconName: "clock-arrow-up"
    contentWidth: 320

    MonthCalendar {
        width: parent.width
        ink: root.ink
    }
}
