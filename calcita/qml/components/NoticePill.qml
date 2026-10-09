import QtQuick
import org.celestina.calcita 1.0

// A short message near the bottom of a window: a refused drop, a page that
// does not exist. It fades after a few seconds.
Item {
    id: pill

    property string kind: "info"
    property string text: ""

    function show(kind, text) {
        pill.kind = kind
        pill.text = text
        hideTimer.restart()
    }

    objectName: "notice"
    visible: opacity > 0
    opacity: hideTimer.running ? 1 : 0
    implicitWidth: label.implicitWidth + CelestinaTheme.spaceLg * 2
    implicitHeight: label.implicitHeight + CelestinaTheme.spaceSm * 2
    Accessible.role: Accessible.AlertMessage
    Accessible.name: pill.text

    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionNormal
        }
    }

    CelestinaSurface {
        anchors.fill: parent
        role: CelestinaSurface.Elevated
        radiusOverride: CelestinaTheme.radiusPill
    }

    Text {
        id: label
        objectName: "noticeText"
        anchors.centerIn: parent
        text: pill.text
        color: pill.kind === "error" ? CelestinaTheme.danger : CelestinaTheme.text
        font.pixelSize: CelestinaTheme.fontBody
    }

    Timer {
        id: hideTimer
        interval: 4000
    }
}
