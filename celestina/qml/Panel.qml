import QtQuick
import QtQuick.Window
import CelestinaStyle

Window {
    id: panel

    width: Screen.width
    height: 40
    visible: false
    // Translucent glass tint so the compositor's blur behind the panel
    // (enableBlurBehind, main.cpp) reads through it. A compositor without the
    // effect just shows this tint over the wallpaper.
    color: CelestinaTheme.glassTint
    title: qsTr("Celestina Panel")
    flags: Qt.FramelessWindowHint | Qt.WindowDoesNotAcceptFocus

    Clock {
        anchors.centerIn: parent
    }

    // The phone, when Magnetita has one connected. Hidden otherwise — no daemon,
    // no device, and the panel is just the clock.
    Row {
        id: phoneIndicator

        anchors.right: parent.right
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        spacing: 6
        visible: Phone.phoneConnected

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
            text: Phone.phoneName
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: 13
        }

        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 3
            visible: Phone.phoneBattery >= 0

            Image {
                anchors.verticalCenter: parent.verticalCenter
                visible: Phone.phoneCharging
                source: "qrc:/qt/qml/CelestinaDesktop/battery-charging.svg"
                sourceSize: Qt.size(15, 15)
                width: 15
                height: 15
                smooth: true
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Phone.phoneBattery + " %"
                color: Phone.phoneBattery <= 15 ? CelestinaTheme.danger
                     : Phone.phoneBattery <= 30 ? CelestinaTheme.warning
                     : CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.features: CelestinaTheme.fontFeaturesTabular
                font.pixelSize: 13
            }
        }
    }

    Rectangle {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        height: 1
        color: CelestinaTheme.divider
    }
}
