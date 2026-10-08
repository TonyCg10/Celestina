import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The format dialog: it opens on exFAT, quick, this volume only and the
// current name; FAT32 writes the name in capitals as it is typed; a name the
// file system cannot hold shows why under the field and keeps Formatear off;
// erasing the whole disk names the disk, not the partition; Formatear hands the
// controller exactly the device and UUID captured at open plus the choices;
// a stick unplugged while the dialog is up disables Formatear; Escape closes
// without a call. The stub controller never touches a device.
TestCase {
    id: testCase
    name: "FormatDialog"
    width: 640
    height: 720
    visible: true
    when: windowShown

    property var formatCalls: []
    property int focusReturns: 0

    QtObject {
        id: controllerStub

        property var volumeNames: ["AAA", "USB"]
        property var volumeDevices: ["/dev/sdc1", "/dev/sdb1"]
        property var volumeUuids: ["AAAA-0001", "1234-ABCD"]
        property var volumeLabels: ["AAA", "Fotos"]
        property var volumeSizes: ["8000000000", "16008609792"]
        property var volumeDisks: ["/dev/sdc", "/dev/sdb"]
        property var volumeDiskSizes: ["8004304896", "16009658368"]
        property bool volumeBusy: false

        // The FAT32 rules the Rust side applies after uppercasing, enough to
        // tell a valid name from one that is not.
        function labelError(fs, label) {
            if (fs === "vfat" && label.toUpperCase().length > 11)
                return "En FAT32 el nombre admite como mucho 11 caracteres"
            if (fs === "vfat" && label.indexOf("*") >= 0)
                return "En FAT32 el nombre no admite «*»"
            return ""
        }
        function formatVolume(device, uuid, fs, label, quick, wholeDisk) {
            testCase.formatCalls.push({ device: device, uuid: uuid, fs: fs,
                                        label: label, quick: quick,
                                        wholeDisk: wholeDisk })
        }
    }

    QtObject {
        id: ownerStub
        property int width: 640
        property int height: 720
        function focusView() { testCase.focusReturns++ }
    }

    Item {
        id: backdropStub
        width: 640
        height: 720
    }

    FormatDialog {
        id: dialog
        anchors.fill: parent
        controller: controllerStub
        owner: ownerStub
        backdrop: backdropStub
    }

    function child(name) {
        const item = findChild(dialog, name)
        verify(item !== null, "no " + name)
        return item
    }

    function init() {
        testCase.formatCalls = []
        testCase.focusReturns = 0
        controllerStub.volumeDevices = ["/dev/sdc1", "/dev/sdb1"]
        controllerStub.volumeUuids = ["AAAA-0001", "1234-ABCD"]
        controllerStub.volumeBusy = false
        dialog.shown = false
    }

    function openStick() {
        dialog.openFor("/dev/sdb1")
        tryCompare(dialog, "opacity", 1)
    }

    function test_a_opens_on_exfat_quick_this_volume_and_the_current_name() {
        openStick()
        compare(dialog.shown, true)
        compare(dialog.fsType, "exfat")
        compare(dialog.quick, true)
        compare(dialog.wholeDisk, false)
        compare(child("formatName").text, "Fotos")
        compare(child("formatToggle-quick").checked, true)
        compare(child("formatToggle-wholeDisk").checked, false)
        verify(child("formatCapacity").text.indexOf("GiB") > 0, child("formatCapacity").text)
        compare(child("formatConfirm").enabled, true)
        // The focus is parked, not placed on a control: no ring on open.
        compare(child("formatName").activeFocus, false)
        // The second open starts from the defaults again.
        mouseClick(child("fs-ntfs"))
        mouseClick(child("formatToggle-quick"))
        mouseClick(child("formatToggle-wholeDisk"))
        compare(dialog.fsType, "ntfs")
        compare(dialog.quick, false)
        compare(dialog.wholeDisk, true)
        dialog.dismiss()
        openStick()
        compare(dialog.fsType, "exfat")
        compare(dialog.quick, true)
        compare(dialog.wholeDisk, false)
    }

    function test_b_fat32_uppercases_the_name_and_refuses_an_invalid_one() {
        openStick()
        mouseClick(child("fs-vfat"))
        compare(dialog.fsType, "vfat")
        const field = child("formatName")
        // Choosing FAT32 writes the current name the way it will be stored.
        compare(field.text, "FOTOS")
        field.selectAll()
        field.forceActiveFocus()
        keyClick(Qt.Key_Backspace)
        keyClick(Qt.Key_M)
        keyClick(Qt.Key_I)
        keyClick(Qt.Key_Space)
        keyClick(Qt.Key_U)
        compare(field.text, "MI U")
        compare(child("formatLabelError").visible, false)
        compare(child("formatConfirm").enabled, true)
        // Twelve characters: too long for FAT32; the reason shows, Formatear goes off.
        field.text = ""
        keyClick(Qt.Key_A)
        for (let i = 0; i < 11; ++i)
            keyClick(Qt.Key_B)
        compare(field.text, "ABBBBBBBBBBB")
        compare(child("formatLabelError").visible, true)
        compare(child("formatLabelError").text,
                "En FAT32 el nombre admite como mucho 11 caracteres")
        compare(child("formatConfirm").enabled, false)
        dialog.confirm()
        compare(testCase.formatCalls.length, 0, "an invalid name was sent")
        // Back to exFAT, the same name is fine.
        mouseClick(child("fs-exfat"))
        compare(child("formatLabelError").visible, false)
        compare(child("formatConfirm").enabled, true)
    }

    function test_c_whole_disk_names_the_disk() {
        openStick()
        const warning = child("formatWarning")
        verify(warning.text.indexOf("/dev/sdb1") >= 0, warning.text)
        mouseClick(child("formatToggle-wholeDisk"))
        compare(dialog.wholeDisk, true)
        verify(warning.text.indexOf("/dev/sdb ") >= 0, warning.text)
        verify(warning.text.indexOf("/dev/sdb1") < 0, warning.text)
    }

    function test_d_formatear_sends_exactly_what_was_chosen() {
        openStick()
        mouseClick(child("fs-ext4"))
        mouseClick(child("formatToggle-quick"))
        mouseClick(child("formatToggle-wholeDisk"))
        const field = child("formatName")
        field.text = "Copias"
        // A hotplug re-sorts the list while the dialog is up: the call still
        // names the stick it opened on.
        controllerStub.volumeDevices = ["/dev/sdb1", "/dev/sdc1"]
        controllerStub.volumeUuids = ["1234-ABCD", "AAAA-0001"]
        mouseClick(child("formatConfirm"))
        compare(testCase.formatCalls.length, 1)
        const call = testCase.formatCalls[0]
        compare(call.device, "/dev/sdb1")
        compare(call.uuid, "1234-ABCD")
        compare(call.fs, "ext4")
        compare(call.label, "Copias")
        compare(call.quick, false)
        compare(call.wholeDisk, true)
        // It closes on start; progress is the notice's.
        compare(dialog.shown, false)
        compare(testCase.focusReturns, 1)
    }

    function test_e_the_stick_going_away_disables_formatear() {
        openStick()
        compare(child("formatConfirm").enabled, true)
        controllerStub.volumeDevices = ["/dev/sdc1"]
        controllerStub.volumeUuids = ["AAAA-0001"]
        compare(child("formatConfirm").enabled, false)
        // The warning gives way to the reason (the "no longer connected" copy).
        compare(child("formatWarning").text, qsTr("La unidad ya no está conectada"))
        dialog.confirm()
        compare(testCase.formatCalls.length, 0)
        // Another stick under the same device node is not the one chosen.
        controllerStub.volumeDevices = ["/dev/sdc1", "/dev/sdb1"]
        controllerStub.volumeUuids = ["AAAA-0001", "9999-0000"]
        compare(child("formatConfirm").enabled, false)
        dialog.confirm()
        compare(testCase.formatCalls.length, 0)
        // A drive operation running elsewhere keeps it off too.
        controllerStub.volumeUuids = ["AAAA-0001", "1234-ABCD"]
        compare(child("formatConfirm").enabled, true)
        controllerStub.volumeBusy = true
        compare(child("formatConfirm").enabled, false)
    }

    function test_f_escape_closes_without_a_call() {
        openStick()
        keyClick(Qt.Key_Escape)
        compare(dialog.shown, false)
        compare(testCase.formatCalls.length, 0)
        // From the name field too.
        openStick()
        child("formatName").forceActiveFocus()
        keyClick(Qt.Key_Escape)
        compare(dialog.shown, false)
        compare(testCase.formatCalls.length, 0)
        // And Cancelar.
        openStick()
        mouseClick(child("formatCancel"))
        compare(dialog.shown, false)
        compare(testCase.formatCalls.length, 0)
    }

    function test_g_fat32_below_its_minimum_is_refused() {
        controllerStub.volumeSizes = ["8000000000", "33554432"]
        controllerStub.volumeDiskSizes = ["8004304896", "34603008"]
        openStick()
        mouseClick(child("fs-vfat"))
        compare(child("formatSizeError").visible, true)
        compare(child("formatConfirm").enabled, false)
        mouseClick(child("fs-exfat"))
        compare(child("formatConfirm").enabled, true)
        controllerStub.volumeSizes = ["8000000000", "16008609792"]
        controllerStub.volumeDiskSizes = ["8004304896", "16009658368"]
    }
}
