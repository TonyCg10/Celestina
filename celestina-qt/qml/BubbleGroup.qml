// Compact shell presence for Niri's native minimized windows.
//
// This is one group, never a running-app dock. The overlap says that several
// live windows are held behind one entry point; the selector owns identity and
// actions after this button is opened.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

PanelMenuButton {
    id: root

    required property var reading
    signal selectorRequested(rect openerRect, rect attachmentAnchorRect)

    readonly property var windows: reading !== undefined
                                   && reading.available === true
                                   && reading.windows !== undefined
                                   ? reading.windows : []
    readonly property int bubbleCount: windows.length
    readonly property int visibleBubbleCount: Math.min(3, bubbleCount)
    readonly property int bubbleSize: 22
    readonly property int overlapStep: 9

    objectName: "celestina-bubble-group"
    visible: bubbleCount > 0
    implicitWidth: stack.implicitWidth
    implicitHeight: CelestinaTheme.controlHeightXs
    attachmentAnchor: frontBubble

    Accessible.role: Accessible.Button
    Accessible.name: qsTr("%n ventana(s) minimizada(s)", "", bubbleCount)
    Accessible.description: qsTr("Abrir las burbujas de aplicaciones")
    Accessible.onPressAction: root.requestMenu()
    onMenuRequested: (openerRect, attachmentAnchorRect) =>
        root.selectorRequested(openerRect, attachmentAnchorRect)

    contentItem: Item {
        id: stack

        implicitWidth: root.bubbleCount > 0
                       ? root.bubbleSize
                         + (root.visibleBubbleCount - 1) * root.overlapStep
                       : 0
        implicitHeight: root.bubbleSize

        Repeater {
            model: root.visibleBubbleCount

            delegate: Rectangle {
                id: bubble

                required property int index
                objectName: "celestina-bubble"
                readonly property var windowEntry: root.windows[
                    root.visibleBubbleCount - 1 - index]
                readonly property string iconIdentity:
                    windowEntry.iconName !== undefined
                    && windowEntry.iconName.length > 0
                    ? windowEntry.iconName
                    : windowEntry.appId !== undefined ? windowEntry.appId : ""

                x: index * root.overlapStep
                anchors.verticalCenter: parent.verticalCenter
                width: root.bubbleSize
                height: width
                radius: width / 2
                color: root.ink.selectedRestFill
                z: index

                Image {
                    id: applicationIcon

                    anchors.centerIn: parent
                    width: 16
                    height: width
                    sourceSize: Qt.size(32, 32)
                    fillMode: Image.PreserveAspectFit
                    asynchronous: false
                    visible: status === Image.Ready
                    source: bubble.iconIdentity.length > 0
                            ? "image://appicon/"
                              + encodeURIComponent(bubble.iconIdentity)
                            : ""
                }

                CelestinaIcon {
                    anchors.centerIn: parent
                    width: 16
                    height: width
                    visible: !applicationIcon.visible
                    name: "app-window"
                    fallbackName: "app-window"
                    tintOverride: root.ink.primary
                    Accessible.ignored: true
                }
            }
        }

        Item {
            id: frontBubble

            x: Math.max(0, (root.visibleBubbleCount - 1) * root.overlapStep)
            anchors.verticalCenter: parent.verticalCenter
            width: root.bubbleSize
            height: width
        }

        Rectangle {
            objectName: "celestina-bubble-overflow"
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            width: 13
            height: width
            radius: width / 2
            visible: root.bubbleCount > root.visibleBubbleCount
            color: root.ink.accent
            z: 10

            Text {
                anchors.centerIn: parent
                text: "+" + (root.bubbleCount - root.visibleBubbleCount)
                color: CelestinaTheme.accentInk
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
                font.weight: CelestinaTheme.weightDemiBold
            }
        }
    }
}
