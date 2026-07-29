import CelestinaStyle
import QtQuick

Item {
    id: root

    property string timeString: ""

    function updateClock() {
        const now = new Date();
        root.timeString = Qt.formatTime(now, "HH:mm");
        const elapsedMinute = now.getSeconds() * 1000 + now.getMilliseconds();
        minuteTimer.interval = Math.max(250, 60000 - elapsedMinute);
        minuteTimer.restart();
    }

    implicitWidth: clockText.implicitWidth
    implicitHeight: clockText.implicitHeight
    Component.onCompleted: updateClock()

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
        font.pixelSize: CelestinaTheme.fontRowTitle
        font.weight: CelestinaTheme.weightMedium
    }

}
