pragma Singleton
import QtQuick

// The sections, in the order the strip shows them. The window, the strip's
// shortcuts and the tests all read this one list.
QtObject {
    readonly property var all: [
        { id: "network", title: qsTr("Red"), icon: "wifi" },
        { id: "bluetooth", title: qsTr("Bluetooth"), icon: "bluetooth" },
        { id: "audio", title: qsTr("Audio"), icon: "media-volume" },
        { id: "appearance", title: qsTr("Apariencia"), icon: "paintbrush" }
    ]
}
