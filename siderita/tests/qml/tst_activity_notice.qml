import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// One transient announcement. The rules that matter are about time: nothing
// appears before 500 ms, an action that settles inside that window is never
// drawn at all, a notice that was drawn mutates into its settled wording and
// then retires on its own, and a press retires it at once without reaching the
// file it covers.
TestCase {
    id: testCase
    name: "ActivityNotice"
    width: 600
    height: 400
    visible: true
    when: windowShown

    property var dismissals: []
    property int contentPresses: 0

    Item {
        id: backdropStub
        anchors.fill: parent
    }

    // What lies under the notice in the folder: a row delegate's MouseArea.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        hoverEnabled: true
        onPressed: testCase.contentPresses++
    }

    Component {
        id: noticeFactory
        ActivityNotice {
            backdrop: backdropStub
            maxWidth: 400
            onDismissed: id => testCase.dismissals.push(id)
        }
    }

    function init() {
        testCase.dismissals = []
        testCase.contentPresses = 0
        mouseMove(testCase, 590, 10)
    }

    function test_a_running_notice_is_not_drawn_before_the_threshold() {
        const notice = noticeFactory.createObject(testCase, {
            noticeId: "1", text: "Desmontando…", iconName: "unplug",
            danger: false, running: true, y: 300
        })
        compare(notice.shown, false, "nothing is drawn immediately")
        wait(300)
        compare(notice.shown, false, "still nothing at 300 ms")
        tryVerify(function() { return notice.shown }, 1000,
                  "it appears once the action has lasted past the threshold")
        notice.destroy()
    }

    function test_an_action_that_settles_inside_the_threshold_is_never_drawn() {
        const notice = noticeFactory.createObject(testCase, {
            noticeId: "2", text: "Desmontando…", iconName: "unplug",
            danger: false, running: true, y: 300
        })
        wait(200)
        notice.running = false
        compare(notice.shown, false, "a 200 ms unmount never flashes")
        compare(testCase.dismissals, ["2"],
                "and it asks to be dropped rather than lingering")
        notice.destroy()
    }

    function test_an_outcome_is_drawn_at_once_and_retires_on_its_own() {
        const notice = noticeFactory.createObject(testCase, {
            noticeId: "3", text: "Pegado cancelado", iconName: "circle-stop",
            danger: false, running: false, y: 300
        })
        compare(notice.shown, true,
                "an announcement about something already finished does not wait")
        tryVerify(function() { return testCase.dismissals.length === 1 }, 8000,
                  "and it retires without being touched")
        compare(testCase.dismissals, ["3"])
        notice.destroy()
    }

    function test_a_press_retires_it_and_never_reaches_the_content() {
        const notice = noticeFactory.createObject(testCase, {
            noticeId: "4", text: "Disco desmontado", iconName: "unplug",
            danger: false, running: false, y: 300
        })
        compare(notice.shown, true)
        mouseClick(notice, notice.width / 2, notice.height / 2)
        compare(testCase.contentPresses, 0,
                "the press stops at the notice instead of selecting a file")
        tryVerify(function() { return testCase.dismissals.length === 1 }, 1000)
        notice.destroy()
    }

    function test_a_danger_notice_wraps_but_an_info_notice_does_not() {
        const long = "No se pudo leer la carpeta: permiso denegado en "
                   + "/run/media/toni/disco/una/ruta/larga/de/verdad"
        const info = noticeFactory.createObject(testCase, {
            noticeId: "5", text: long, iconName: "info",
            danger: false, running: false, y: 100
        })
        const bad = noticeFactory.createObject(testCase, {
            noticeId: "6", text: long, iconName: "circle-alert",
            danger: true, running: false, y: 200
        })
        verify(info.implicitHeight < bad.implicitHeight,
               "an error may take two lines; an announcement takes one: "
               + info.implicitHeight + " vs " + bad.implicitHeight)
        verify(bad.implicitWidth <= bad.maxWidth,
               "and neither is ever wider than the space it was given")
        info.destroy()
        bad.destroy()
    }
}
