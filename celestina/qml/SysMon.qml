// CPU and memory, as the helper read them from `/proc`.
//
// It shows what it has and nothing else: before the second sample there is no
// rate yet, and when the provider goes away its value goes with it, so this
// widget simply disappears rather than freezing on a number from a minute ago.
import CelestinaStyle
import QtQuick

Item {
    id: root

    // The `sysmon` provider's fields, or `undefined` while it carries none.
    // `var` is necessary because QML has no typed map.
    required property var reading
    // A click opens the system monitor the session already uses. Like every
    // other action here it is a request: the host sends it and reports back.
    signal monitorRequested()

    readonly property bool hasReading: reading !== undefined && reading.cpu !== undefined

    function loadColor(load) {
        if (load === "critical")
            return CelestinaTheme.danger;

        if (load === "elevated")
            return CelestinaTheme.warning;

        return CelestinaTheme.textMuted;
    }

    implicitWidth: hasReading ? readings.implicitWidth : 0
    implicitHeight: 26
    visible: hasReading
    Accessible.role: Accessible.Button
    Accessible.name: hasReading
                     ? qsTr("Procesador %1 %, memoria %2 %").arg(reading.cpu).arg(reading.ram)
                     : ""
    Accessible.description: qsTr("Abre el monitor del sistema")
    Accessible.onPressAction: root.monitorRequested()

    Row {
        id: readings

        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceMd

        Text {
            text: root.hasReading ? qsTr("CPU %1 %").arg(root.reading.cpu) : ""
            color: root.loadColor(root.hasReading ? root.reading.cpuLoad : "")
            font.family: CelestinaTheme.sansFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontCaption
        }

        Text {
            text: root.hasReading ? qsTr("RAM %1 %").arg(root.reading.ram) : ""
            color: root.loadColor(root.hasReading ? root.reading.ramLoad : "")
            font.family: CelestinaTheme.sansFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontCaption
        }

    }

    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.monitorRequested()
    }

}
