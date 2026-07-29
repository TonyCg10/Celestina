import CelestinaStyle
import QtQuick

Row {
    id: root

    required property bool connected
    required property string phoneName
    required property int battery
    required property bool charging

    spacing: 6
    visible: connected
    Accessible.role: Accessible.StaticText
    Accessible.name: battery >= 0 ? qsTr("%1, batería %2 por ciento").arg(phoneName).arg(battery) : phoneName

    Image {
        anchors.verticalCenter: parent.verticalCenter
        source: "qrc:/qt/qml/CelestinaDesktop/phone.svg"
        sourceSize: Qt.size(15, 15)
        width: 15
        height: 15
        smooth: true
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: root.phoneName
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
        elide: Text.ElideRight
    }

    Row {
        anchors.verticalCenter: parent.verticalCenter
        spacing: 3
        visible: root.battery >= 0

        Image {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.charging
            source: "qrc:/qt/qml/CelestinaDesktop/battery-charging.svg"
            sourceSize: Qt.size(15, 15)
            width: 15
            height: 15
            smooth: true
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.battery + " %"
            color: root.battery <= 15 ? CelestinaTheme.danger : root.battery <= 30 ? CelestinaTheme.warning : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontBody
        }

    }

}
