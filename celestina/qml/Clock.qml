import CelestinaStyle
import QtQuick

Item {
    id: root

    // The lived format: time with seconds, the month, then the weekday and day.
    // The panel's language is Spanish by construction, so the month and weekday
    // names are asked for in it rather than inherited from whatever locale the
    // process happened to start with — a shell launched from a C-locale service
    // would otherwise print an English date into a Spanish panel.
    readonly property var uiLocale: Qt.locale("es_ES")
    readonly property string format: "HH:mm:ss - MMMM - dddd dd"
    property string timeString: ""

    // Aligned to the boundary of the smallest unit shown, so the display never
    // sits a fraction of a second behind the clock it claims to be.
    function updateClock() {
        const now = new Date();
        root.timeString = now.toLocaleString(root.uiLocale, root.format);
        tick.interval = Math.max(50, 1000 - now.getMilliseconds());
        tick.restart();
    }

    implicitWidth: clockText.implicitWidth
    implicitHeight: clockText.implicitHeight
    Accessible.role: Accessible.StaticText
    Accessible.name: timeString
    Component.onCompleted: updateClock()

    Timer {
        id: tick

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
