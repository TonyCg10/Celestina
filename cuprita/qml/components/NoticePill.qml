import QtQuick
import org.celestina.cuprita 1.0

// The window's notice: one short sentence from a controller, shown for a few
// seconds at the bottom. An error reads in the danger ink.
Item {
    id: pill

    property string kind: ""
    property string text: ""

    function show(kind, text) {
        pill.kind = kind
        pill.text = text
        hide.restart()
    }

    implicitWidth: capsule.implicitWidth
    implicitHeight: capsule.implicitHeight
    visible: opacity > 0
    opacity: hide.running ? 1 : 0
    Accessible.role: Accessible.AlertMessage
    Accessible.name: pill.text

    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionNormal
        }
    }

    Timer {
        id: hide
        interval: 4000
    }

    CelestinaCapsule {
        id: capsule
        inset: CelestinaTheme.spaceSm

        Text {
            objectName: "noticeText"
            text: pill.text
            color: pill.kind === "error" ? CelestinaTheme.danger : CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            leftPadding: CelestinaTheme.spaceSm
            rightPadding: CelestinaTheme.spaceSm
        }
    }
}
