// A monitor's workspaces folded into one compact control. The visible monitor
// name was redundant on the author's fixed layout, so the capsule keeps only
// the count, its interaction and urgent state; the technical name remains in
// its accessible label.
import CelestinaStyle
import QtQuick

Item {
    id: capsule

    // The monitor this group belongs to; assistive context, not visible text.
    required property string outputName
    required property int count
    required property BackdropInk ink
    // Whether anything inside is asking for attention.
    required property bool urgent
    signal expandRequested()
    signal mapRequested(int globalX, int globalY)

    objectName: "celestina-workspace-group-" + outputName
    implicitWidth: Math.max(30, countLabel.implicitWidth + CelestinaTheme.spaceMd)
    implicitHeight: 26
    Accessible.role: Accessible.Button
    Accessible.name: urgent
                     ? qsTr("%1, %2 espacios, requiere atención").arg(outputName).arg(count)
                     : qsTr("%1, %2 espacios").arg(outputName).arg(count)
    Accessible.description: qsTr("Muestra los espacios de este monitor")
    Accessible.onPressAction: capsule.expandRequested()

    Rectangle {
        id: feedback
        objectName: "celestina-workspace-group-feedback"
        anchors.fill: parent
        radius: CelestinaTheme.radiusPill
        color: pressArea.pressed ? capsule.ink.pressedFill
                                 : pressArea.containsMouse
                                   ? capsule.ink.hoverFill
                                   : CelestinaTheme.clear
        border.width: CelestinaTheme.borderHairline
        border.color: capsule.urgent ? capsule.ink.danger : capsule.ink.focus

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
            }
        }
    }

    Text {
        id: countLabel

        anchors.centerIn: parent
        text: capsule.count
        color: capsule.ink.primary
        font.family: CelestinaTheme.sansFamily
        font.features: CelestinaTheme.fontFeaturesTabular
        font.pixelSize: CelestinaTheme.fontTitle
        Accessible.ignored: true
    }

    Rectangle {
        anchors.top: parent.top
        anchors.right: parent.right
        width: 5
        height: 5
        radius: CelestinaTheme.radiusPill
        visible: capsule.urgent
        color: capsule.ink.danger
    }

    MouseArea {
        id: pressArea
        objectName: "celestina-workspace-group-pointer"

        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        onClicked: (mouse) => {
            if (mouse.button === Qt.RightButton) {
                const anchor = capsule.mapToGlobal(0, capsule.height);
                capsule.mapRequested(anchor.x, anchor.y);
                return;
            }
            capsule.expandRequested();
        }
    }

}
