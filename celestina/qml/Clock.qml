import QtQuick
import CelestinaStyle

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
        color: CelestinaTheme.text
        font.family: CelestinaTheme.monoFamily
        font.features: CelestinaTheme.fontFeaturesTabular
        font.pixelSize: 15
        font.weight: CelestinaTheme.weightMedium
    }

    Component.onCompleted: updateClock()
}
