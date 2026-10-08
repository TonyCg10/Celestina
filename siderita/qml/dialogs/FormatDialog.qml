pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

    // ── Format a removable volume ─────────────────────────────────────
    // The Explorer-style format dialog: what is erased (device and size), the
    // file system, the new name, quick or zero-filled, and whether the whole
    // drive becomes one new partition. It decides nothing about the disk: the
    // controller re-validates everything and refuses a system drive.
    //
    // The target is captured when the dialog opens — the device node and the
    // filesystem UUID it held — and both travel to `formatVolume`, so a stick
    // swapped or reformatted under the same node is refused rather than
    // erased. Every live column is read by device, never by row: a hotplug
    // re-sorts the list while the dialog is up.
CelestinaModalLayer {
    id: format
    // Required rather than typed: the tests hand in stubs with the same
    // members, which a `SideritaController` or `Item` type would refuse.
    required property var controller
    // The surface the card sizes itself against, and gives focus back to.
    required property var owner
    // What the glass samples behind the card (the main panel).
    required property Item backdrop
    anchors.fill: parent
    z: 61
    onDismissRequested: format.dismiss()

    // The target, as it was when the dialog opened.
    property string device: ""
    property string uuid: ""
    property string volumeName: ""
    property real size: 0
    property string disk: ""
    property real diskSize: 0

    // The choices; `openFor` resets them to exFAT, quick, this volume only.
    property string fsType: "exfat"
    property alias quick: quickRow.checked
    property alias wholeDisk: wholeDiskRow.checked

    readonly property var fileSystems: [
        { token: "exfat", label: "exFAT",
          hint: qsTr("Compatible con Windows, macOS, TV y cámaras") },
        { token: "vfat", label: "FAT32",
          hint: qsTr("Aparatos antiguos; archivos de hasta 4 GB") },
        { token: "ntfs", label: "NTFS", hint: qsTr("Para Windows") },
        { token: "ext4", label: "ext4", hint: qsTr("Solo Linux") },
        { token: "btrfs", label: "btrfs", hint: qsTr("Solo Linux, con instantáneas") }
    ]

    // FAT32's floor, as the Rust side enforces it (`drives::FAT32_MIN`).
    readonly property real fat32Minimum: 64 * 1024 * 1024
    // A wiped disk's one partition starts 1 MiB in (`drives::PARTITION_OFFSET`).
    readonly property real partitionOffset: 1024 * 1024

    // Where the target sits in the live listing now, -1 once it is gone.
    readonly property int row: {
        const devices = format.controller.volumeDevices
        return devices ? devices.indexOf(format.device) : -1
    }
    readonly property bool present: format.device.length > 0 && format.row >= 0
    // Still the content the person chose: the same filesystem UUID.
    readonly property bool unchanged: format.present
                                      && format.controller.volumeUuids[format.row] === format.uuid
    readonly property bool canWholeDisk: format.disk.length > 0 && format.diskSize > 0
    readonly property string eraseDevice: format.wholeDisk ? format.disk : format.device
    readonly property real eraseSize: format.wholeDisk ? format.diskSize : format.size
    readonly property string labelError: format.controller.labelError(format.fsType, nameField.text)
    readonly property bool tooSmall: format.fsType === "vfat"
                                     && (format.wholeDisk ? format.diskSize - format.partitionOffset
                                                          : format.size) < format.fat32Minimum
    readonly property string blocker: !format.present ? qsTr("La unidad ya no está conectada")
                                      : !format.unchanged ? qsTr("La unidad ha cambiado desde que se abrió este diálogo")
                                      : ""
    readonly property bool canFormat: format.shown && format.blocker.length === 0
                                      && format.labelError.length === 0 && !format.tooSmall
                                      && !format.controller.volumeBusy

    // The size formatter Siderita shows a volume's capacity with (the
    // properties panel's free/total line: `FolderUsage.bytesText`).
    function bytesText(bytes) {
        if (bytes >= 1099511627776) return (bytes / 1099511627776).toLocaleString(Qt.locale(), "f", 1) + " TiB"
        if (bytes >= 1073741824) return (bytes / 1073741824).toLocaleString(Qt.locale(), "f", 1) + " GiB"
        if (bytes >= 1048576) return (bytes / 1048576).toLocaleString(Qt.locale(), "f", 1) + " MiB"
        if (bytes >= 1024) return (bytes / 1024).toLocaleString(Qt.locale(), "f", 0) + " KiB"
        return qsTr("%1 B").arg(bytes)
    }

    // A FAT label is stored in capitals; the field shows it that way as it is
    // typed. The Rust side normalizes again and decides.
    function normalized(text) {
        return format.fsType === "vfat" ? text.toUpperCase() : text
    }
    function normalizeField() {
        const upper = format.normalized(nameField.text)
        if (upper === nameField.text)
            return
        const cursor = nameField.cursorPosition
        nameField.text = upper
        nameField.cursorPosition = Math.min(cursor, upper.length)
    }

    // A switch with the text that names it; the text toggles it too.
    component ToggleRow: Item {
        id: toggleRow

        required property string key
        required property string label
        property alias checked: toggle.checked

        height: Math.max(toggle.implicitHeight, toggleLabel.implicitHeight)

        Text {
            id: toggleLabel
            anchors.left: parent.left
            anchors.right: toggle.left
            anchors.rightMargin: CelestinaTheme.spaceMd
            anchors.verticalCenter: parent.verticalCenter
            text: toggleRow.label
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: toggleRow.enabled ? CelestinaTheme.text : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody

            MouseArea {
                anchors.fill: parent
                onClicked: toggle.toggle()
            }
        }

        CelestinaSwitch {
            id: toggle
            objectName: "formatToggle-" + toggleRow.key
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            Accessible.name: toggleRow.label
        }
    }

    function openFor(device) {
        const devices = format.controller.volumeDevices
        const index = devices ? devices.indexOf(device) : -1
        if (index < 0)
            return
        const column = function(list) { return list && index < list.length ? list[index] : "" }
        format.device = device
        format.uuid = column(format.controller.volumeUuids)
        format.volumeName = column(format.controller.volumeNames)
        format.size = Number(column(format.controller.volumeSizes)) || 0
        format.disk = column(format.controller.volumeDisks)
        format.diskSize = Number(column(format.controller.volumeDiskSizes)) || 0
        format.fsType = "exfat"
        format.quick = true
        format.wholeDisk = false
        nameField.text = column(format.controller.volumeLabels)
        format.shown = true
    }
    function chooseFileSystem(token) {
        format.fsType = token
        format.normalizeField()
    }
    function dismiss() {
        format.shown = false
        format.owner.focusView()
    }
    function confirm() {
        if (!format.canFormat)
            return
        format.controller.formatVolume(format.device, format.uuid, format.fsType,
                                       format.normalized(nameField.text),
                                       format.quick, format.wholeDisk)
        format.dismiss()
    }

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(460, format.owner.width - 48)
        height: Math.min(content.implicitHeight + CelestinaTheme.spaceLg * 2,
                         format.owner.height - 48)
        backdropSource: format.backdrop
        Accessible.role: Accessible.Dialog
        Accessible.name: heading.text

        // Swallow clicks so they never reach the dismiss backdrop.
        MouseArea { anchors.fill: parent }

        Column {
            id: content
            x: CelestinaTheme.spaceLg
            y: CelestinaTheme.spaceLg
            width: parent.width - CelestinaTheme.spaceLg * 2
            spacing: CelestinaTheme.spaceMd

            Column {
                width: parent.width
                spacing: CelestinaTheme.spaceXs

                Text {
                    id: heading
                    width: parent.width
                    text: qsTr("Formatear «%1»").arg(format.volumeName)
                    textFormat: Text.PlainText
                    elide: Text.ElideMiddle
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowTitle
                    font.weight: CelestinaTheme.weightDemiBold
                }

                Text {
                    objectName: "formatCapacity"
                    width: parent.width
                    text: qsTr("Capacidad: %1").arg(format.bytesText(format.size))
                    textFormat: Text.PlainText
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowSecondary
                }
            }

            // Five choices, each with the line that tells them apart: a
            // vertical list, since five segments with their hints do not fit
            // the card. One Tab stop; Up/Down walk it, like a radio group.
            FocusScope {
                id: fsList
                objectName: "formatFileSystems"
                width: parent.width
                height: fsColumn.implicitHeight
                activeFocusOnTab: true
                Accessible.role: Accessible.List
                Accessible.name: qsTr("Sistema de archivos")

                function move(delta) {
                    const count = format.fileSystems.length
                    let index = 0
                    for (let i = 0; i < count; ++i)
                        if (format.fileSystems[i].token === format.fsType)
                            index = i
                    format.chooseFileSystem(format.fileSystems[(index + delta + count) % count].token)
                }
                Keys.onUpPressed: function(event) { fsList.move(-1); event.accepted = true }
                Keys.onDownPressed: function(event) { fsList.move(1); event.accepted = true }

                Column {
                    id: fsColumn
                    width: parent.width

                    Repeater {
                        model: format.fileSystems

                        AbstractButton {
                            id: fsRow

                            required property int index
                            required property var modelData
                            readonly property bool current: modelData.token === format.fsType

                            objectName: "fs-" + modelData.token
                            width: fsColumn.width
                            implicitHeight: fsText.implicitHeight + CelestinaTheme.spaceSm * 2
                            hoverEnabled: true
                            focusPolicy: current ? Qt.TabFocus : Qt.NoFocus
                            focus: current
                            Accessible.role: Accessible.RadioButton
                            Accessible.name: modelData.label
                            Accessible.description: modelData.hint
                            Accessible.checked: current
                            onClicked: format.chooseFileSystem(modelData.token)

                            background: Rectangle {
                                radius: CelestinaTheme.radiusSm
                                color: fsRow.current ? CelestinaTheme.surfaceSelected
                                       : fsRow.hovered ? CelestinaTheme.surfaceHover
                                       : CelestinaTheme.clear

                                CelestinaFocusRing {
                                    target: parent
                                    cornerRadius: parent.radius
                                    shown: fsRow.visualFocus
                                }
                            }

                            contentItem: Item {
                                implicitHeight: fsText.implicitHeight

                                Column {
                                    id: fsText
                                    x: CelestinaTheme.spaceMd
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: parent.width - CelestinaTheme.spaceMd * 2

                                    Text {
                                        width: parent.width
                                        text: fsRow.modelData.label
                                        textFormat: Text.PlainText
                                        color: CelestinaTheme.text
                                        font.family: CelestinaTheme.sansFamily
                                        font.pixelSize: CelestinaTheme.fontBody
                                        font.weight: fsRow.current ? CelestinaTheme.weightDemiBold
                                                                   : CelestinaTheme.weightRegular
                                    }
                                    Text {
                                        width: parent.width
                                        text: fsRow.modelData.hint
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        color: CelestinaTheme.textMuted
                                        font.family: CelestinaTheme.sansFamily
                                        font.pixelSize: CelestinaTheme.fontRowSecondary
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Text {
                objectName: "formatSizeError"
                width: parent.width
                visible: format.tooSmall
                text: qsTr("FAT32 necesita un volumen de al menos 64 MiB")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: CelestinaTheme.danger
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
            }

            Column {
                width: parent.width
                spacing: CelestinaTheme.spaceXs

                Text {
                    text: qsTr("Nombre")
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowSecondary
                }

                CelestinaTextField {
                    id: nameField
                    objectName: "formatName"
                    width: parent.width
                    Accessible.name: qsTr("Nombre del volumen")
                    onTextEdited: format.normalizeField()
                    onAccepted: format.confirm()
                    Keys.onPressed: function(event) {
                        if (event.key === Qt.Key_Escape) {
                            format.dismiss()
                            event.accepted = true
                        }
                    }
                }

                Text {
                    objectName: "formatLabelError"
                    width: parent.width
                    visible: format.labelError.length > 0
                    text: format.labelError
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: CelestinaTheme.danger
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowSecondary
                }
            }

            ToggleRow {
                id: quickRow
                width: parent.width
                key: "quick"
                label: qsTr("Formato rápido")
            }

            ToggleRow {
                id: wholeDiskRow
                width: parent.width
                key: "wholeDisk"
                label: qsTr("Borrar todo el disco y crear una sola partición")
                // Only when the disk under the volume is known.
                enabled: format.canWholeDisk
            }

            // What is erased, by name and size; or why nothing can be.
            Text {
                objectName: "formatWarning"
                width: parent.width
                text: format.blocker.length > 0
                      ? format.blocker
                      : qsTr("Se borrará todo lo que hay en %1 (%2).")
                            .arg(format.eraseDevice).arg(format.bytesText(format.eraseSize))
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: CelestinaTheme.danger
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                font.weight: CelestinaTheme.weightMedium
            }

            Item {
                width: parent.width
                height: buttons.implicitHeight

                Row {
                    id: buttons
                    anchors.right: parent.right
                    spacing: CelestinaTheme.spaceSm

                    CelestinaButton {
                        objectName: "formatCancel"
                        text: qsTr("Cancelar")
                        onClicked: format.dismiss()
                    }
                    CelestinaButton {
                        objectName: "formatConfirm"
                        text: qsTr("Formatear")
                        role: CelestinaButton.Destructive
                        enabled: format.canFormat
                        onClicked: format.confirm()
                    }
                }
            }
        }
    }
}
