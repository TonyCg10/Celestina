import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The compress dialog: the name is composed by the controller (from the bytes
// of the selection and of the folder), not by QML, and changing the container
// asks for it again — unless the person has typed their own, which is never
// overwritten. What is sent is the selection as-is, the name and the format.
TestCase {
    id: testCase
    name: "CompressDialog"
    width: 480
    height: 360
    visible: true
    when: windowShown

    property var compressCalls: []
    property var suggestCalls: []

    QtObject {
        id: controllerStub

        // El nombre sugerido depende del formato, como en el controlador real.
        function archiveSuggestedName(keys, format) {
            testCase.suggestCalls.push({ keys: keys, format: format })
            return "proyecto." + format
        }
        function compressKeys(keys, name, format) {
            testCase.compressCalls.push({ keys: keys, name: name, format: format })
        }
    }

    property int focusReturns: 0
    QtObject {
        id: ownerStub
        property int width: 480
        function focusView() { testCase.focusReturns++ }
    }

    Item {
        id: backdropStub
        width: 480
        height: 360
    }

    CompressDialog {
        id: dialog
        anchors.fill: parent
        controller: controllerStub
        owner: ownerStub
        backdrop: backdropStub
    }

    function init() {
        testCase.compressCalls = []
        testCase.suggestCalls = []
        testCase.focusReturns = 0
        dialog.shown = false
    }

    function test_a_opens_with_the_name_the_controller_composed() {
        dialog.openFor(["clave:uno", "clave:dos"])
        compare(dialog.shown, true)
        compare(dialog.targets.length, 2)
        compare(dialog.format, "zip")
        compare(testCase.suggestCalls.length, 1)
        compare(testCase.suggestCalls[0].format, "zip")
    }

    function test_b_changing_the_container_asks_for_the_name_again() {
        dialog.openFor(["clave:uno"])
        dialog.chooseFormat("tar.gz")
        compare(dialog.format, "tar.gz")
        // It asked for a new one for the new container (plus the earlier check).
        compare(testCase.suggestCalls[testCase.suggestCalls.length - 1].format,
                "tar.gz")
    }

    function test_c_confirming_sends_the_selection_the_name_and_the_format() {
        dialog.openFor(["clave:uno", "clave:dos"])
        dialog.confirm()
        compare(testCase.compressCalls.length, 1)
        compare(testCase.compressCalls[0].keys.length, 2)
        compare(testCase.compressCalls[0].name, "proyecto.zip")
        compare(testCase.compressCalls[0].format, "zip")
        // Y devuelve el foco a la vista, como cualquier modal de esta carpeta.
        compare(dialog.shown, false)
        compare(testCase.focusReturns, 1)
    }

    function test_d_cancelling_compresses_nothing() {
        dialog.openFor(["clave:uno"])
        dialog.dismiss()
        compare(testCase.compressCalls.length, 0)
        compare(dialog.shown, false)
        compare(dialog.targets.length, 0)
    }

    function test_e_an_empty_selection_never_opens_the_dialog() {
        dialog.openFor([])
        compare(dialog.shown, false)
        compare(testCase.suggestCalls.length, 0)
    }
}
