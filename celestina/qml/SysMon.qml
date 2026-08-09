// CPU and memory, as the helper read them from `/proc`.
//
// It shows what it has and nothing else: before the second sample there is no
// rate yet, and when the provider goes away its value goes with it, so this
// widget simply disappears rather than freezing on a number from a minute ago.
pragma ComponentBehavior: Bound

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

    implicitWidth: hasReading ? readings.implicitWidth : 0
    implicitHeight: 26
    visible: hasReading
    Accessible.role: Accessible.Button
    Accessible.name: hasReading
                     ? qsTr("Procesador %1 %, memoria %2 %").arg(reading.cpu).arg(reading.ram)
                     : ""
    Accessible.description: qsTr("Abre el monitor del sistema")
    Accessible.onPressAction: root.monitorRequested()

    component Metric: Row {
        required property string iconName
        required property string value

        spacing: CelestinaTheme.spaceXs

        CelestinaIcon {
            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.iconSm
            height: CelestinaTheme.iconSm
            name: parent.iconName
            tone: CelestinaIcon.Primary
            Accessible.ignored: true
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: parent.value
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontTitle
        }
    }

    Row {
        id: readings

        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceMd

        Metric {
            objectName: "celestina-cpu-icon"
            iconName: "cpu"
            value: root.hasReading ? qsTr("%1 %").arg(root.reading.cpu) : ""
        }

        Metric {
            objectName: "celestina-memory-icon"
            iconName: "memory-stick"
            value: root.hasReading ? qsTr("%1 %").arg(root.reading.ram) : ""
        }

    }

    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.monitorRequested()
    }

}
