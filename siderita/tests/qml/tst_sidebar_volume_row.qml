import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// Renaming a removable volume in place: Return hands the label to the
// controller and ends the edit, Escape ends it without a call, a label the
// file system cannot hold keeps the field open with its reason, a FAT label is
// shown in capitals as it will be stored, and F2 on the focused row starts it —
// except on a drive of the running system, where it never starts at all.
//
// The second half rebuilds the list in the middle of an edit, as a hotplug
// does: the rename must stay on the device it started on, keep what was typed,
// and reach the controller with that device, never with whatever now sorts
// into the row's old position.
TestCase {
    id: testCase
    name: "SidebarVolumeRow"
    width: 320
    height: 400
    visible: true
    when: windowShown

    property string renamingDevice: ""

    QtObject {
        id: controllerStub

        property string markedKey: ""
        property var volumeDevices: ["/dev/sdb1", "/dev/sda2"]
        property var volumeMounts: ["/run/media/toni/USB", ""]
        property var volumeLabels: ["USB", "DATOS"]
        property var volumeFsTypes: ["vfat", "exfat"]
        property var volumeSystem: ["0", "1"]
        property var renames: []
        property int opens: 0

        // The FAT32 rule the Rust side applies after uppercasing, enough to
        // tell the row apart.
        function labelError(fs, label) {
            if (fs !== "vfat" && fs !== "exfat")
                return "not renamable"
            if (fs === "vfat" && label.toUpperCase().length > 11)
                return "too long"
            return ""
        }
        function renameVolume(device, label) { renames.push({ device: device, label: label }) }
        function openVolume(index) { opens++ }
        function unmountVolume(index) { }
    }

    QtObject {
        id: hostWindowStub

        property var activeController: controllerStub
        property real sidebarIconScale: 1.0
        property real sidebarTextScale: 1.0
        property int sidebarRowHeight: 34

        function openTab(path, foreground) { }
    }

    SidebarVolumeRow {
        id: usb
        x: 10
        y: 10
        width: 280
        hostWindow: hostWindowStub
        overlayParent: testCase
        rowIndex: 0
        volumeName: "USB"
        device: "/dev/sdb1"
        editing: testCase.renamingDevice === device
        onEditRequested: function(device) { testCase.renamingDevice = device }
        onEditFinished: testCase.renamingDevice = ""
    }

    SidebarVolumeRow {
        id: systemDisk
        x: 10
        y: 60
        width: 280
        hostWindow: hostWindowStub
        overlayParent: testCase
        rowIndex: 1
        volumeName: "DATOS"
        device: "/dev/sda2"
        editing: testCase.renamingDevice === device
        onEditRequested: function(device) { testCase.renamingDevice = device }
        onEditFinished: testCase.renamingDevice = ""
    }

    // ── The sidebar's own wiring, over a list that a hotplug rebuilds ───────

    QtObject {
        id: liveController

        property string markedKey: ""
        property var volumeNames: ["USB"]
        property var volumeDevices: ["/dev/sdb1"]
        property var volumeMounts: [""]
        property var volumeLabels: ["USB"]
        property var volumeFsTypes: ["exfat"]
        property var volumeSystem: ["0"]
        property var renames: []

        function labelError(fs, label) { return fs === "exfat" ? "" : "not renamable" }
        function renameVolume(device, label) { renames.push({ device: device, label: label }) }
        function openVolume(index) { }
        function unmountVolume(index) { }

        // A stick that sorts before "USB" is plugged in: the details first,
        // the names last, as the controller publishes them.
        function plugFirst() {
            volumeDevices = ["/dev/sdc1", "/dev/sdb1"]
            volumeMounts = ["", ""]
            volumeLabels = ["AAA", "USB"]
            volumeFsTypes = ["exfat", "exfat"]
            volumeSystem = ["0", "0"]
            volumeNames = ["AAA", "USB"]
        }
        function unplugTheStick() {
            volumeDevices = ["/dev/sdc1"]
            volumeMounts = [""]
            volumeLabels = ["AAA"]
            volumeFsTypes = ["exfat"]
            volumeSystem = ["0"]
            volumeNames = ["AAA"]
        }
    }

    QtObject {
        id: liveHost

        property var activeController: liveController
        property real sidebarIconScale: 1.0
        property real sidebarTextScale: 1.0
        property int sidebarRowHeight: 34

        function openTab(path, foreground) { }
    }

    Column {
        id: liveColumn
        x: 10
        y: 200
        width: 280

        property string renamingDevice: ""
        property var renamingDraft: null

        Repeater {
            id: liveRows
            model: liveController.volumeNames
            delegate: SidebarVolumeRow {
                required property int index
                required property string modelData
                width: liveColumn.width
                hostWindow: liveHost
                overlayParent: testCase
                rowIndex: index
                volumeName: modelData
                device: index < liveController.volumeDevices.length
                        ? liveController.volumeDevices[index] : ""
                editing: device.length > 0 && liveColumn.renamingDevice === device
                draft: liveColumn.renamingDraft
                onEditRequested: function(device) {
                    liveColumn.renamingDraft = null
                    liveColumn.renamingDevice = device
                }
                onEditFinished: liveColumn.renamingDevice = ""
                onDraftEdited: function(text) { liveColumn.renamingDraft = text }
            }
        }
    }

    function init() {
        renamingDevice = ""
        controllerStub.renames = []
        controllerStub.opens = 0
        liveController.renames = []
        liveController.volumeNames = ["USB"]
        liveController.volumeDevices = ["/dev/sdb1"]
        liveController.volumeMounts = [""]
        liveController.volumeLabels = ["USB"]
        liveController.volumeFsTypes = ["exfat"]
        liveController.volumeSystem = ["0"]
        liveColumn.renamingDevice = ""
        liveColumn.renamingDraft = null
    }

    function startRenaming() {
        usb.forceActiveFocus()
        keyClick(Qt.Key_F2)
        tryVerify(function() { return usb.editing }, 1000, "F2 did not start the rename")
    }

    function typeLabel(text) {
        keyClick(Qt.Key_A, Qt.ControlModifier)
        keyClick(Qt.Key_Backspace)
        for (let i = 0; i < text.length; i++)
            keyClick(text[i])
    }

    function rowFor(device) {
        for (let i = 0; i < liveRows.count; i++) {
            const row = liveRows.itemAt(i)
            if (row && row.device === device)
                return row
        }
        return null
    }

    function test_a_return_commits_the_label() {
        startRenaming()
        typeLabel("FOTOS")
        keyClick(Qt.Key_Return)
        compare(controllerStub.renames.length, 1, "Return did not rename")
        compare(controllerStub.renames[0].device, "/dev/sdb1")
        compare(controllerStub.renames[0].label, "FOTOS")
        verify(!usb.editing, "the edit stayed open after a valid rename")
        compare(controllerStub.opens, 0, "the commit's Return also opened the volume")
    }

    function test_b_escape_cancels() {
        startRenaming()
        typeLabel("OTRO")
        keyClick(Qt.Key_Escape)
        verify(!usb.editing, "Escape did not end the edit")
        compare(controllerStub.renames.length, 0, "Escape renamed")
    }

    function test_c_an_invalid_label_keeps_the_editor_with_its_reason() {
        startRenaming()
        typeLabel("vacaciones 2026")
        keyClick(Qt.Key_Return)
        compare(controllerStub.renames.length, 0, "an invalid label was renamed")
        verify(usb.editing, "the edit closed on an invalid label")
        compare(usb.errorText, "too long")
        verify(usb.height > hostWindowStub.sidebarRowHeight, "the reason is not shown")

        // The reason follows the typing, and a fixed label goes through.
        typeLabel("FOTOS")
        compare(usb.errorText, "")
        keyClick(Qt.Key_Return)
        compare(controllerStub.renames.length, 1)
        compare(controllerStub.renames[0].label, "FOTOS")
    }

    function test_d_a_fat_label_is_committed_in_capitals() {
        startRenaming()
        typeLabel("fotos")
        keyClick(Qt.Key_Return)
        compare(controllerStub.renames.length, 1)
        compare(controllerStub.renames[0].label, "FOTOS",
                "the FAT label did not go in capitals")
    }

    function test_e_the_same_label_is_no_rename() {
        startRenaming()
        keyClick(Qt.Key_Return)
        verify(!usb.editing)
        compare(controllerStub.renames.length, 0, "an unchanged label was sent")
    }

    function test_f_a_system_drive_never_starts_renaming() {
        verify(!systemDisk.canRename)
        systemDisk.forceActiveFocus()
        keyClick(Qt.Key_F2)
        wait(50)
        verify(!systemDisk.editing, "F2 renamed a system drive")
        compare(renamingDevice, "")
    }

    function test_g_return_on_the_focused_row_still_opens_it() {
        usb.forceActiveFocus()
        keyClick(Qt.Key_Return)
        compare(controllerStub.opens, 1)
    }

    function test_h_a_hotplug_mid_rename_keeps_the_device_and_the_text() {
        const stick = rowFor("/dev/sdb1")
        verify(stick !== null)
        stick.forceActiveFocus()
        keyClick(Qt.Key_F2)
        tryVerify(function() { return stick.editing }, 1000)
        typeLabel("Fotos")

        liveController.plugFirst()
        // The stick's row is now the second one; the first is someone else's.
        compare(liveColumn.renamingDevice, "/dev/sdb1", "the rebuild ended the rename")
        const moved = rowFor("/dev/sdb1")
        verify(moved !== null)
        compare(moved.rowIndex, 1)
        verify(moved.editing, "the rename did not follow the device")
        verify(!rowFor("/dev/sdc1").editing, "the rename jumped to the new stick")
        // The new row's field takes the focus and the typed text: Return
        // there commits it.
        tryVerify(function() { return moved.fieldFocused }, 1000,
                  "the new row's field did not take the focus")
        keyClick(Qt.Key_Return)
        compare(liveController.renames.length, 1)
        compare(liveController.renames[0].device, "/dev/sdb1",
                "the rename reached another drive")
        compare(liveController.renames[0].label, "Fotos", "the typed text was lost")
    }

    function test_i_a_stick_unplugged_mid_rename_takes_the_edit_with_it() {
        const stick = rowFor("/dev/sdb1")
        stick.forceActiveFocus()
        keyClick(Qt.Key_F2)
        tryVerify(function() { return stick.editing }, 1000)
        typeLabel("Fotos")

        liveController.unplugTheStick()
        compare(liveRows.count, 1)
        verify(!rowFor("/dev/sdc1").editing, "the other stick inherited the rename")
        compare(liveController.renames.length, 0, "a rename went out for a missing stick")
    }
}
