// The lived format: time with seconds, the month, then the weekday and day —
// and the way in to the calendar.
//
// A real `PanelMenuButton`, not a plain item with a click: the attachment
// lease resolves the drop's anchor by walking the panel for marked openers,
// and a surface whose opener is not one is deliberately left floating. The
// clock grew a hand-rolled `menuRequested` first, and its calendar opened
// with no connection to the bar — the lease could not find it.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

PanelMenuButton {
    id: root

    // The panel's language is Spanish by construction, so the month and
    // weekday names are asked for in it rather than inherited from whatever
    // locale the process happened to start with — a shell launched from a
    // C-locale service would otherwise print an English date into a Spanish
    // panel.
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

    attachmentAnchor: clockText
    leftPadding: CelestinaTheme.spaceSm
    rightPadding: CelestinaTheme.spaceSm
    implicitWidth: clockText.implicitWidth + leftPadding + rightPadding
    Accessible.name: timeString
    Accessible.description: qsTr("Abre el calendario")
    Component.onCompleted: updateClock()

    Timer {
        id: tick

        repeat: false
        onTriggered: root.updateClock()
    }

    contentItem: Item {
        implicitWidth: clockText.implicitWidth
        implicitHeight: clockText.implicitHeight

        Text {
            id: clockText

            anchors.centerIn: parent
            text: root.timeString
            color: root.ink.primary
            font.family: CelestinaTheme.monoFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontTitle
            font.weight: CelestinaTheme.weightDemiBold
        }
    }
}
