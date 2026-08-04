import QtQuick
import org.celestina.fluorita 1.0

// Stepping through a folder while a video plays or a track sounds.
//
// A filmstrip is for looking: it earns its space by letting you *choose* the
// next picture from thumbnails you can read at a glance. There is nothing to
// read at a glance for a video or an audio track — a strip of posters over a
// playing film is furniture in front of the thing you came for. So those get
// what they actually need, which is previous and next.
//
// Each arrow appears when the pointer comes near its own edge, and both appear
// whenever either holds focus, so Tab never lands on something invisible.
Item {
    id: arrows

    required property ContentNavigator navigator

    signal stepped(int step)

    anchors.fill: parent

    QtObject {
        id: metrics

        // The band each arrow answers to. Wide enough to find without hunting,
        // narrow enough to leave the middle of the picture alone.
        readonly property int reach: CelestinaTheme.spaceLg * 8
        readonly property int travel: CelestinaTheme.reducedMotion
            ? 0 : CelestinaTheme.motionNormal
    }

    Item {
        id: leftEdge

        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: metrics.reach

        HoverHandler { id: leftApproach }

        CelestinaIconButton {
            id: previous

            anchors.left: parent.left
            anchors.leftMargin: CelestinaTheme.spaceLg
            anchors.verticalCenter: parent.verticalCenter
            activeFocusOnTab: arrows.navigator.hasPrevious
            visible: arrows.navigator.navigable && arrows.navigator.hasPrevious
            enabled: visible
            iconName: "go-previous"
            fallbackIcon: "chevron-left"
            iconSize: CelestinaTheme.iconMd
            role: CelestinaButton.Tonal
            density: CelestinaButton.Regular
            Accessible.name: qsTr("Anterior")
            opacity: leftApproach.hovered || previous.activeFocus || next.activeFocus ? 1 : 0

            Behavior on opacity {
                NumberAnimation {
                    duration: metrics.travel
                    easing.type: CelestinaTheme.easeStandard
                }
            }

            onClicked: arrows.stepped(-1)
        }
    }

    Item {
        id: rightEdge

        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: metrics.reach

        HoverHandler { id: rightApproach }

        CelestinaIconButton {
            id: next

            anchors.right: parent.right
            anchors.rightMargin: CelestinaTheme.spaceLg
            anchors.verticalCenter: parent.verticalCenter
            activeFocusOnTab: arrows.navigator.hasNext
            visible: arrows.navigator.navigable && arrows.navigator.hasNext
            enabled: visible
            iconName: "go-next"
            fallbackIcon: "chevron-right"
            iconSize: CelestinaTheme.iconMd
            role: CelestinaButton.Tonal
            density: CelestinaButton.Regular
            Accessible.name: qsTr("Siguiente")
            opacity: rightApproach.hovered || next.activeFocus || previous.activeFocus ? 1 : 0

            Behavior on opacity {
                NumberAnimation {
                    duration: metrics.travel
                    easing.type: CelestinaTheme.easeStandard
                }
            }

            onClicked: arrows.stepped(1)
        }
    }
}
