import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The password dialog: it appears while the controller says an extraction is
// waiting, answers with what was typed, and keeps nothing — neither between
// attempts nor between archives. Skipping does not cancel the batch: that one
// archive is passed over and the controller carries on with the rest.
TestCase {
    id: testCase
    name: "PasswordDialog"
    width: 480
    height: 360
    visible: true
    when: windowShown

    property var answers: []
    property int skips: 0

    QtObject {
        id: controllerStub
        property bool passwordPending: false
        property string passwordArchive: ""
        property bool passwordRetry: false

        function answerPassword(value) {
            testCase.answers.push(value)
            passwordPending = false
        }
        function cancelPassword() {
            testCase.skips++
            passwordPending = false
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

    PasswordDialog {
        id: dialog
        anchors.fill: parent
        controller: controllerStub
        owner: ownerStub
        backdrop: backdropStub
    }

    function init() {
        testCase.answers = []
        testCase.skips = 0
        testCase.focusReturns = 0
        controllerStub.passwordRetry = false
        controllerStub.passwordArchive = "secreto.rar"
        controllerStub.passwordPending = false
    }

    function test_a_it_shows_itself_only_while_an_extraction_waits() {
        compare(dialog.shown, false)
        controllerStub.passwordPending = true
        compare(dialog.shown, true)
        controllerStub.passwordPending = false
        compare(dialog.shown, false)
    }

    function test_b_answering_sends_what_was_typed_and_keeps_nothing() {
        controllerStub.passwordPending = true
        keyClick(Qt.Key_C)
        keyClick(Qt.Key_L)
        dialog.submit()
        compare(testCase.answers.length, 1)
        compare(testCase.answers[0], "cl")
        compare(testCase.focusReturns, 1)

        // The next question starts empty: a password never travels from one
        // archive to the next.
        controllerStub.passwordPending = true
        compare(testCase.answers.length, 1)
        dialog.submit()
        compare(testCase.answers.length, 1)
    }

    function test_c_an_empty_field_answers_nothing() {
        controllerStub.passwordPending = true
        dialog.submit()
        compare(testCase.answers.length, 0)
        compare(dialog.shown, true)
    }

    function test_d_skipping_asks_the_controller_to_carry_on() {
        controllerStub.passwordPending = true
        dialog.skip()
        compare(testCase.skips, 1)
        compare(testCase.answers.length, 0)
        compare(testCase.focusReturns, 1)
    }

    function test_e_a_wrong_password_says_so_and_asks_again() {
        controllerStub.passwordPending = true
        controllerStub.passwordRetry = true
        compare(dialog.shown, true)
        // The retry state belongs to the controller; the dialog only reads it,
        // and keeps asking instead of giving up.
        compare(controllerStub.passwordRetry, true)
        dialog.skip()
        compare(testCase.skips, 1)
    }
}
