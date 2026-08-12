// A monitor's workspaces folded into one compact control. The author removed
// the bordered capsule and its visible count on 2026-08-11: the collapsed
// group is now a single dot, deliberately larger than the workspace dots
// beside it, so the strip reads as one family of points. The count and the
// technical monitor name remain in the accessible label.
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
    signal mapRequested(rect openerRect, rect attachmentAnchorRect)

    // The semantic attachment-source contract the panel lease resolves: the
    // control is the placement opener and its dot is the exact glyph the
    // contextual droplet's mouth follows.
    readonly property bool isPanelAttachmentSource: true
    readonly property Item attachmentAnchor: groupMark
    // Set only by the tokened attachment lease that owns the currently mapped
    // contextual surface; the dot keeps its hover emphasis until that exact
    // surface retires.
    property bool menuOpen: false

    function globalRect(item) {
        const topLeft = item.mapToGlobal(0, 0);
        const bottomRight = item.mapToGlobal(item.width, item.height);
        return Qt.rect(Math.min(topLeft.x, bottomRight.x),
                       Math.min(topLeft.y, bottomRight.y),
                       Math.abs(bottomRight.x - topLeft.x),
                       Math.abs(bottomRight.y - topLeft.y));
    }

    function attachmentAnchorGlobalRectNow() {
        return capsule.globalRect(capsule.attachmentAnchor);
    }

    objectName: "celestina-workspace-group-" + outputName
    implicitWidth: 24
    implicitHeight: 26
    Accessible.role: Accessible.Button
    Accessible.name: urgent
                     ? qsTr("%1, %2 espacios, requiere atención").arg(outputName).arg(count)
                     : qsTr("%1, %2 espacios").arg(outputName).arg(count)
    Accessible.description: qsTr("Muestra los espacios de este monitor")
    Accessible.onPressAction: capsule.expandRequested()

    Rectangle {
        id: groupMark
        objectName: "celestina-workspace-group-mark"

        anchors.centerIn: parent
        // Larger than the 10..12 px workspace dots beside it: one whole
        // monitor behind one point.
        width: 16
        height: width
        radius: width / 2
        scale: pressArea.pressed ? 0.82 : 1
        color: capsule.urgent ? capsule.ink.danger : capsule.ink.primary
        opacity: pressArea.pressed ? CelestinaTheme.disabledContentOpacity
                 : pressArea.containsMouse || capsule.menuOpen ? 0.82 : 1

        Behavior on scale {
            NumberAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }
    }

    Rectangle {
        anchors.top: groupMark.top
        anchors.right: groupMark.right
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
                capsule.mapRequested(capsule.globalRect(capsule),
                                     capsule.attachmentAnchorGlobalRectNow());
                return;
            }
            capsule.expandRequested();
        }
    }

}
