import QtQuick

Item {
    id: root

    implicitWidth: clockText.implicitWidth
    implicitHeight: clockText.implicitHeight

    property string timeString: ""

    function updateClock() {
        const now = new Date()
        root.timeString = Qt.formatTime(now, "HH:mm")

        const elapsedMinute = now.getSeconds() * 1000 + now.getMilliseconds()
        minuteTimer.interval = Math.max(250, 60000 - elapsedMinute)
        minuteTimer.restart()
    }

    Timer {
        id: minuteTimer

        repeat: false
        onTriggered: root.updateClock()
    }

    Text {
        id: clockText

        text: root.timeString
        color: "#e0def4"
        font.family: "monospace"
        font.pixelSize: 15
        font.weight: Font.Medium
    }

    Component.onCompleted: updateClock()
}
