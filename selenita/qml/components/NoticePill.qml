import QtQuick
import org.celestina.selenita 1.0

// The window's one notice: a short sentence from the controller, held for a
// few seconds near the bottom edge. An error is set in the danger ink.
Item {
    id: pill

    property string kind: ""
    property string text: ""

    function show(kind: string, text: string) {
        pill.kind = kind
        pill.text = text
        holder.restart()
    }

    implicitWidth: capsule.implicitWidth
    implicitHeight: capsule.implicitHeight
    opacity: holder.running ? 1 : 0
    visible: opacity > 0
    Accessible.role: Accessible.AlertMessage
    Accessible.name: pill.text

    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionNormal
        }
    }

    Timer {
        id: holder
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
