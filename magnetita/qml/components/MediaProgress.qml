import QtQuick
import org.celestina.magnetita 1.0

Item {
    id: root

    required property real value
    property string accessibleDescription: ""

    readonly property real normalizedValue: Math.max(0, Math.min(1, root.value))

    implicitHeight: 24
    Accessible.role: Accessible.ProgressBar
    Accessible.name: "Progreso de reproducción"
    Accessible.description: root.accessibleDescription

    Rectangle {
        id: track

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        height: CelestinaTheme.compLinearTrackHeight
        radius: height / 2
        color: CelestinaTheme.mediaProgressTrack
        Accessible.ignored: true
    }

    Rectangle {
        anchors.left: track.left
        anchors.verticalCenter: track.verticalCenter
        width: track.width * root.normalizedValue
        height: track.height
        radius: height / 2
        color: CelestinaTheme.mediaProgress
        Accessible.ignored: true
    }

    Rectangle {
        width: CelestinaTheme.compSliderHandleSize
        height: width
        radius: height / 2
        x: Math.max(0, Math.min(root.width - width,
                                root.width * root.normalizedValue - width / 2))
        anchors.verticalCenter: parent.verticalCenter
        color: CelestinaTheme.mediaProgress
        visible: root.normalizedValue > 0
        Accessible.ignored: true
    }
}
