import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

Item {
    id: root

    required property var hostWindow
    required property Item overlayParent
    required property int rowIndex
    required property string bookmarkName
    required property string bookmarkPath
    required property bool editing
    required property int listDragIndex
    required property int listDropIndex
    required property int rowPitch
    required property int rowCount

    signal editRequested(int index)
    signal editCancelled
    signal renameRequested(int index, string value)
    signal dragMoved(int dropIndex)
    signal dragFinished(int from, int to)
    signal dragCancelled
    signal contextMenuRequested(int index, string path, real popupX, real popupY)

    readonly property bool current: bookmarkPath.length > 0
                                    && bookmarkPath === (hostWindow.activeController
                                                         ? hostWindow.activeController.markedKey : "")
    readonly property bool dragging: listDragIndex === rowIndex
    property bool justDragged: false
    property bool dragConsumed: false

    height: hostWindow.sidebarRowHeight
    z: dragging ? 2 : 0
    // The same keyboard path the phone rows have: Tab reaches the row and
    // Return opens it. Not while renaming — the field owns the keys then.
    activeFocusOnTab: !root.editing
    Accessible.role: Accessible.Button
    Accessible.name: root.bookmarkName
    Accessible.onPressAction: root.activate()

    function activate() {
        if (root.hostWindow.activeController)
            root.hostWindow.activeController.openKey(root.bookmarkPath)
    }

    Keys.onPressed: function(event) {
        // The rename field's own Return travels up this chain too; only a key
        // pressed on the focused row is an activation.
        if (!root.activeFocus)
            return
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            root.activate()
            event.accepted = true
        }
    }

    // A single click opens the bookmark, but only once the double-click window
    // has passed: the second click of a double click is the rename gesture, and
    // opening on the first one navigated away before the field ever appeared.
    Timer {
        id: singleClick
        interval: Application.styleHints.mouseDoubleClickInterval
        onTriggered: root.activate()
    }

    Rectangle {
        z: 3
        visible: root.listDragIndex >= 0
                 && root.listDragIndex !== root.rowIndex
                 && root.listDropIndex === root.rowIndex
        x: 2
        width: parent.width - 4
        height: CelestinaTheme.compDragIndicatorHeight
        radius: height / 2
        y: root.listDropIndex > root.listDragIndex ? parent.height - height : 0
        color: CelestinaTheme.accent
    }

    Item {
        id: rowContent
        width: root.width
        height: root.height
        opacity: root.dragging ? CelestinaTheme.draggedContentOpacity : 1

        Behavior on y {
            enabled: !rowMouse.drag.active
            NumberAnimation {
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }

        Rectangle {
            anchors.fill: parent
            anchors.leftMargin: 2
            anchors.rightMargin: 2
            radius: CelestinaTheme.radiusSm
            border.width: root.activeFocus ? CelestinaTheme.borderFocus : 0
            border.color: CelestinaTheme.focusRing
            color: root.dragging
                   ? CelestinaTheme.surfaceStrong
                   : root.current
                     ? CelestinaTheme.badgeAccentFill
                     : rowMouse.containsMouse
                       ? CelestinaTheme.surfaceHover
                       : CelestinaTheme.clear

            Behavior on color {
                ColorAnimation { duration: CelestinaTheme.motionFast }
            }
        }

        CelestinaIcon {
            id: bookmarkIcon
            x: 12
            anchors.verticalCenter: parent.verticalCenter
            width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
            height: width
            name: "folder"
            fallbackName: "folder"
            tone: CelestinaIcon.Danger
        }

        Text {
            visible: !root.editing
            x: bookmarkIcon.x + bookmarkIcon.width + 10
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - x - 12
            text: root.bookmarkName
            color: root.current ? CelestinaTheme.accent : CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.sidebarTextScale)
            font.weight: root.current ? CelestinaTheme.weightMedium
                                      : CelestinaTheme.weightRegular
            elide: Text.ElideRight
        }

        CelestinaTextField {
            id: editField
            visible: root.editing
            x: bookmarkIcon.x + bookmarkIcon.width + 6
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - x - 8
            height: 26
            text: root.bookmarkName
            font.pixelSize: CelestinaTheme.fontRowSecondary
            leftPadding: CelestinaTheme.spaceSm
            rightPadding: CelestinaTheme.spaceSm
            onVisibleChanged: if (visible) { forceActiveFocus(); selectAll() }
            onAccepted: root.renameRequested(root.rowIndex, text)
            onActiveFocusChanged: {
                if (!activeFocus && root.editing)
                    root.editCancelled()
            }
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    root.editCancelled()
                    event.accepted = true
                }
            }

            // A right button inside the field is not something the text input
            // accepts, so without this it fell past the field to the scene,
            // took its focus with it, and `onActiveFocusChanged` cancelled the
            // rename the person was in the middle of. Swallow it here and hold
            // the caret where it was.
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.RightButton
                onPressed: editField.forceActiveFocus()
            }
        }

        MouseArea {
            id: rowMouse
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
            hoverEnabled: true
            // Declared after the rename field, so it sits on top of it: while
            // editing it steps aside entirely, or a click meant to place the
            // caret opened the bookmark and a right click brought its menu.
            enabled: !root.editing
            cursorShape: root.editing ? Qt.ArrowCursor : Qt.PointingHandCursor
            drag.target: root.editing ? null : rowContent
            drag.axis: Drag.YAxis
            drag.smoothed: false
            drag.threshold: 10
            drag.minimumY: -root.rowIndex * root.rowPitch
            drag.maximumY: (root.rowCount - 1 - root.rowIndex) * root.rowPitch
            preventStealing: true

            onPressed: function() {
                root.dragConsumed = false
                root.justDragged = false
            }

            onPositionChanged: {
                if (!drag.active)
                    return
                root.dragConsumed = true
                const target = Math.max(0, Math.min(root.rowCount - 1,
                                                    root.rowIndex + Math.round(
                                                        rowContent.y / root.rowPitch)))
                root.dragMoved(target)
            }

            onReleased: {
                if (!drag.active || !root.dragConsumed || root.listDragIndex !== root.rowIndex) {
                    rowContent.y = 0
                    root.dragConsumed = false
                    return
                }
                root.justDragged = true
                rowContent.y = 0
                root.dragFinished(root.rowIndex, root.listDropIndex)
            }

            onCanceled: {
                rowContent.y = 0
                root.dragConsumed = false
                if (root.listDragIndex === root.rowIndex)
                    root.dragCancelled()
            }

            onClicked: function(mouse) {
                if (root.justDragged) {
                    root.justDragged = false
                    return
                }
                if (mouse.button === Qt.MiddleButton) {
                    root.hostWindow.openTab(root.bookmarkPath, false)
                } else if (mouse.button === Qt.RightButton) {
                    const point = root.mapToItem(root.overlayParent, mouse.x, mouse.y)
                    root.contextMenuRequested(root.rowIndex, root.bookmarkPath,
                                              point.x, point.y)
                } else {
                    singleClick.restart()
                }
            }

            onDoubleClicked: function(mouse) {
                if (mouse.button !== Qt.LeftButton)
                    return
                singleClick.stop()
                root.editRequested(root.rowIndex)
            }
        }
    }
}
