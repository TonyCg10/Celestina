import QtQuick

// Shared L3 interaction layer: scrim, fade, focus and dismissal semantics.
// Dialog-specific size, content and controller actions stay with the app.
Rectangle {
    id: layer

    property bool shown: false
    property bool dismissOnOutsideClick: true
    property bool dismissOnEscape: true
    signal dismissRequested

    visible: opacity > 0.01
    opacity: shown ? 1 : 0
    color: CelestinaTheme.scrim
    focus: shown

    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.motionFast
            easing.type: CelestinaTheme.easeStandard
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: layer.dismissOnOutsideClick
        onClicked: layer.dismissRequested()
    }

    Keys.onPressed: function(event) {
        if (layer.dismissOnEscape && event.key === Qt.Key_Escape) {
            layer.dismissRequested()
            event.accepted = true
        }
    }
}
