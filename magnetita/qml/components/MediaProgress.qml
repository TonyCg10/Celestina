import QtQuick
import org.celestina.magnetita 1.0

// Where playback is, as a plain bar. Not a slider: the daemon's D-Bus
// contract has no seek verb (`MediaAction` carries only transport words), so
// a thumb here promised a drag nothing could honour. When a seek command
// exists, this becomes a `CelestinaSlider` gated on `mediaCanSeek`.
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
}
