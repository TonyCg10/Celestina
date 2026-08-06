import CelestinaStyle
import QtQuick
import QtQuick.Layouts

RowLayout {
    id: root

    required property bool connected
    required property string phoneName
    required property int battery
    required property bool charging

    spacing: 6
    visible: connected
    Accessible.role: Accessible.StaticText
    Accessible.name: battery >= 0
                     ? qsTr("%1, batería %2 por ciento%3")
                         .arg(phoneName)
                         .arg(battery)
                         .arg(charging ? qsTr(", cargando") : "")
                     : phoneName

    Image {
        Layout.preferredWidth: 15
        Layout.preferredHeight: 15
        source: "qrc:/qt/qml/CelestinaDesktop/phone.svg"
        sourceSize: Qt.size(15, 15)
        smooth: true
        Accessible.ignored: true
    }

    Text {
        Layout.fillWidth: true
        Layout.minimumWidth: 0
        Layout.alignment: Qt.AlignVCenter
        text: root.phoneName
        // The name the paired device reports for itself, shown as characters.
        textFormat: Text.PlainText
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
        elide: Text.ElideRight
        Accessible.ignored: true
    }

    RowLayout {
        Layout.preferredWidth: implicitWidth
        Layout.alignment: Qt.AlignVCenter
        spacing: 3
        visible: root.battery >= 0

        Image {
            Layout.preferredWidth: 15
            Layout.preferredHeight: 15
            visible: root.charging
            source: "qrc:/qt/qml/CelestinaDesktop/battery-charging.svg"
            sourceSize: Qt.size(15, 15)
            smooth: true
            Accessible.ignored: true
        }

        Text {
            Layout.alignment: Qt.AlignVCenter
            text: root.battery + " %"
            color: root.battery <= 15 ? CelestinaTheme.danger : root.battery <= 30 ? CelestinaTheme.warning : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontBody
            Accessible.ignored: true
        }

    }

}
