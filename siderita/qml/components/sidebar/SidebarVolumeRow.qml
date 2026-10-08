import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// One removable volume in DISPOSITIVOS: open it (mounting first), eject it,
// ask for its menu, and rename it in place. The rename edits the row itself,
// like a bookmark's: Return commits, Escape or leaving the field cancels, and a
// label the file system cannot hold keeps the field open with the reason under
// it. F2 on the focused row starts it. A volume of the running system is never
// offered a rename.
//
// The row is the volume whose device node is `device`, not a position: the
// list is sorted by name and rebuilt on every hotplug, so everything it reads
// and every call it makes goes by that device. A rebuild during a rename makes
// a new row for the same device; the host keeps what was typed in `draft` so
// the new row picks it up where the old one left it.
Item {
    id: root

    required property var hostWindow
    required property Item overlayParent
    required property int rowIndex
    required property string volumeName
    required property string device
    required property bool editing
    // What the host kept of an edit in progress; null when none was typed yet.
    property var draft: null

    signal editRequested(string device)
    signal editFinished
    signal draftEdited(string text)
    signal contextMenuRequested(string device, string name, string mountPoint,
                                real popupX, real popupY)

    readonly property var controller: hostWindow.activeController
    readonly property int at: controller && controller.volumeDevices
                              ? controller.volumeDevices.indexOf(device) : -1
    function column(list) {
        return list && at >= 0 && at < list.length ? list[at] : ""
    }
    readonly property string mountPoint: controller ? column(controller.volumeMounts) : ""
    readonly property string label: controller ? column(controller.volumeLabels) : ""
    readonly property string fsType: controller ? column(controller.volumeFsTypes) : ""
    // Unknown counts as system: nothing that changes the drive is offered.
    readonly property bool system: controller ? column(controller.volumeSystem) !== "0" : true
    readonly property bool mounted: mountPoint.length > 0
    readonly property bool current: mounted
        && mountPoint === (controller ? controller.markedKey : "")
    // Renamable when it is not the system's and its file system takes a name
    // at all: the empty name is valid for every file system that does.
    readonly property bool canRename: !system && controller !== null
                                      && controller.labelError(fsType, "") === ""
    // Whether the rename field holds the keyboard (what a test waits for).
    readonly property bool fieldFocused: editField.activeFocus
    // Why the label being typed cannot be used; shown under the field.
    property string errorText: ""

    height: hostWindow.sidebarRowHeight
            + (errorText.length > 0 ? errorLine.implicitHeight + CelestinaTheme.spaceXs : 0)
    // The same keyboard path the phone rows have: Tab reaches the row and
    // Return opens it. Not while renaming — the field owns the keys then.
    activeFocusOnTab: !root.editing
    Accessible.role: Accessible.Button
    Accessible.name: root.volumeName
                     + (root.mounted ? qsTr(", montado") : qsTr(", sin montar"))
    Accessible.onPressAction: root.activate()

    onEditingChanged: if (!editing) errorText = ""
    // A row made while its device is being renamed carries the edit on —
    // after the rebuild settles, since the focus cannot be taken while the
    // row is still being placed and the old row is still being taken down.
    Component.onCompleted: if (editing) Qt.callLater(root.resumeEdit)

    function resumeEdit() {
        if (root.editing)
            root.beginEdit(false)
    }

    function activate() {
        if (root.controller)
            root.controller.openVolume(root.rowIndex)
    }

    // FAT labels are written in capitals, as Windows writes them; the field
    // shows the label as it will be stored. The controller normalizes again
    // and decides.
    function normalized(text) {
        return root.fsType === "vfat" ? text.toUpperCase() : text
    }

    function beginEdit(selectAll) {
        editField.text = (root.draft === null || root.draft === undefined)
                         ? root.label : root.draft
        editField.forceActiveFocus()
        if (selectAll)
            editField.selectAll()
        else
            editField.cursorPosition = editField.text.length
    }

    // Return in the field: the same label is no change, an invalid one keeps
    // the field open with its reason, and a valid one goes to UDisks2.
    function commit(text) {
        const label = root.normalized(text)
        if (label !== text)
            editField.text = label
        if (label === root.label) {
            root.finish()
            return
        }
        const error = root.controller ? root.controller.labelError(root.fsType, label) : ""
        if (error.length > 0) {
            root.errorText = error
            editField.forceActiveFocus()
            return
        }
        root.controller.renameVolume(root.device, label)
        root.finish()
    }

    function finish() {
        root.editFinished()
        root.forceActiveFocus()
    }

    Keys.onPressed: function(event) {
        // The rename field's own keys travel up this chain too; only a key
        // pressed on the focused row is an activation.
        if (!root.activeFocus)
            return
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            root.activate()
            event.accepted = true
        } else if (event.key === Qt.Key_F2 && root.canRename) {
            root.editRequested(root.device)
            event.accepted = true
        }
    }

    Item {
        id: rowBody
        width: root.width
        height: root.hostWindow.sidebarRowHeight

        // The eject glyph is part of the row: the fill holds while the pointer
        // sits on it.
        CelestinaRowHighlight {
            anchors.fill: parent
            anchors.leftMargin: 2
            anchors.rightMargin: 2
            family: CelestinaRowHighlight.Content
            focused: root.activeFocus
            selected: root.current
            hovered: volumeMouse.containsMouse || ejectButton.hovered
            pressed: volumeMouse.pressed
        }

        Rectangle {
            visible: root.current
            x: 2
            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.compSelectionIndicatorWidth
            height: CelestinaTheme.compSelectionIndicatorHeight
            radius: width / 2
            color: CelestinaTheme.accent
        }

        CelestinaIcon {
            id: volumeIcon
            x: 12
            anchors.verticalCenter: parent.verticalCenter
            width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
            height: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
            name: "drive-removable-media"
            fallbackName: "folder"
            tone: root.current ? CelestinaIcon.Accent : CelestinaIcon.Device
        }

        Text {
            visible: !root.editing
            x: volumeIcon.x + volumeIcon.width + 10
            anchors.verticalCenter: parent.verticalCenter
            width: ejectButton.x - x - 6
            text: root.volumeName
            color: root.current ? CelestinaTheme.accentLink : CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.sidebarTextScale)
            elide: Text.ElideRight
        }

        CelestinaTextField {
            id: editField
            visible: root.editing
            x: volumeIcon.x + volumeIcon.width + 6
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - x - 8
            height: 26
            text: root.label
            font.pixelSize: CelestinaTheme.fontRowSecondary
            leftPadding: CelestinaTheme.spaceSm
            rightPadding: CelestinaTheme.spaceSm
            onVisibleChanged: if (visible) root.beginEdit(true)
            onTextChanged: {
                // Only typing is a draft: the text a new row starts with (its
                // label, before the edit resumes) must not overwrite it.
                if (!root.editing || !activeFocus)
                    return
                root.draftEdited(text)
                // Once a label was refused, the reason follows the typing.
                if (root.errorText.length > 0 && root.controller)
                    root.errorText = root.controller.labelError(root.fsType, text)
            }
            // Leaving the field cancels — but not when the row itself is
            // being taken down by a rebuild of the list, which unparents it
            // first: the new row for the same device carries the edit on.
            onActiveFocusChanged: {
                if (!activeFocus && root.editing && root.parent !== null)
                    root.editFinished()
            }
            // Return is taken here rather than through `accepted`: the commit
            // hands the focus back to the row, and an unaccepted Return would
            // then travel on to it and open the volume as well.
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    root.commit(text)
                    event.accepted = true
                } else if (event.key === Qt.Key_Escape) {
                    root.finish()
                    event.accepted = true
                }
            }

            // A right button inside the field would fall through to the scene
            // and take the focus with it, cancelling the rename. Swallow it.
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.RightButton
                onPressed: editField.forceActiveFocus()
            }
        }

        // Eject (unmount) when mounted; hidden otherwise. A ghost icon button
        // at the 30 px floor: the glyph keeps its size, the hover circle and
        // press recoil come with the button.
        CelestinaIconButton {
            id: ejectButton
            z: 3   // above the full-row open handler below
            anchors.verticalCenter: parent.verticalCenter
            anchors.right: parent.right
            anchors.rightMargin: 4
            width: CelestinaTheme.controlHeightXs
            height: CelestinaTheme.controlHeightXs
            iconSize: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
            visible: root.mounted && !root.editing
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: "media-eject"
            helpText: qsTr("Expulsar ") + root.volumeName
            onClicked: {
                if (root.controller)
                    root.controller.unmountVolume(root.rowIndex)
            }
        }

        MouseArea {
            id: volumeMouse
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
            hoverEnabled: true
            // Declared after the rename field, so it sits on top of it: while
            // editing it steps aside, or a click meant to place the caret
            // would open the volume.
            enabled: !root.editing
            cursorShape: Qt.PointingHandCursor
            // Left: open (mounting first if needed) — eject has its own zone.
            // Middle: a background tab, like every other place in this
            // sidebar. Right: the device menu.
            onClicked: function(mouse) {
                if (!root.controller)
                    return
                if (mouse.button === Qt.RightButton) {
                    const point = root.mapToItem(root.overlayParent, mouse.x, mouse.y)
                    root.contextMenuRequested(root.device, root.volumeName,
                                              root.mountPoint, point.x, point.y)
                } else if (mouse.button === Qt.MiddleButton) {
                    // Unmounted there is no path to open, and mounting is
                    // asynchronous: the click does nothing rather than open a
                    // tab to nowhere.
                    if (root.mounted)
                        root.hostWindow.openTab(root.mountPoint, false)
                } else {
                    root.activate()
                }
            }
        }
    }

    Text {
        id: errorLine
        visible: root.errorText.length > 0
        x: editField.x + CelestinaTheme.spaceSm
        y: rowBody.height
        width: root.width - x - 8
        text: root.errorText
        color: CelestinaTheme.danger
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowSecondary
        wrapMode: Text.Wrap
    }
}
