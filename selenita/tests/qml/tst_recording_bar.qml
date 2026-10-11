import QtQuick
import QtTest 1.3
import org.celestina.selenita 1.0

// The recording bar over the stand-in controller: it shows only while a
// recording is under way or paused, it is the frameless, unfocused window
// niri's rule matches by its title, «Pausar»/«Reanudar», «Detener» and
// «Descartar» ask the controller, its clock is the recording card's and
// stands still while paused, a compositor's close is refused while it
// shows, and reduced motion and a pause still the dot.
TestCase {
    id: testCase
    name: "RecordingBar"
    when: windowShown

    Component {
        id: barComponent
        RecordingBar {}
    }

    function findNamed(node, name) {
        if (node.objectName === name)
            return node
        const kids = node.children !== undefined ? node.children : []
        let found = null
        let i = 0
        while (found === null && i < kids.length) {
            found = findNamed(kids[i], name)
            i += 1
        }
        return found
    }

    function named(bar, name) {
        const item = findNamed(bar.contentItem, name)
        verify(item !== null, name)
        return item
    }

    function init() {
        SelenitaController.reset()
        SelenitaController.recordingStem = "Recording"
        CelestinaTheme.reducedMotion = false
    }

    function cleanup() {
        SelenitaController.reset()
        CelestinaTheme.reducedMotion = false
    }

    function makeBar() {
        const bar = createTemporaryObject(barComponent, testCase)
        verify(bar)
        return bar
    }

    // Starts a recording over the stub and waits for the bar's first frame:
    // a pointer event sent before it is lost.
    function record(bar) {
        SelenitaController.toggleRecording()
        verify(bar.visible)
        verify(waitForRendering(bar.contentItem))
    }

    function test_the_window_is_the_one_niri_matches() {
        const bar = makeBar()
        compare(bar.title, qsTr("Grabando"))
        verify(bar.flags & Qt.FramelessWindowHint, "frameless")
        verify(bar.flags & Qt.WindowDoesNotAcceptFocus, "never takes the focus")
        compare(bar.transientParent, null)
    }

    function test_it_shows_only_while_recording() {
        const bar = makeBar()
        verify(!bar.visible, "idle: hidden")
        for (const phase of ["preparing", "stopping", "failed"]) {
            SelenitaController.recordingState = phase
            verify(!bar.visible, phase)
        }
        SelenitaController.recordingState = "recording"
        verify(bar.visible, "recording: shown")
        SelenitaController.recordingState = "stopping"
        verify(!bar.visible, "the stop hides it")
        SelenitaController.recordingState = "recording"
        verify(bar.visible, "the next recording shows it again")
        SelenitaController.recordingState = "paused"
        verify(bar.visible, "paused: still shown")
        SelenitaController.recordingState = "stopping"
        verify(!bar.visible, "a stop from paused hides it at once")
        SelenitaController.recordingState = "idle"
        verify(!bar.visible)
    }

    function test_detener_stops_and_saves() {
        const bar = makeBar()
        record(bar)
        const stop = named(bar, "recordingBarStop")
        compare(stop.text, qsTr("Detener"))
        verify(stop.focusPolicy === Qt.NoFocus)
        mouseClick(stop)
        compare(SelenitaController.calls, ["toggleRecording:false", "stopRecording:recording"])
        compare(SelenitaController.recordingState, "idle")
        compare(SelenitaController.history.length, 1, "the recording is kept")
        verify(!bar.visible)
    }

    function test_descartar_stops_and_keeps_nothing() {
        const bar = makeBar()
        record(bar)
        const discard = named(bar, "recordingBarDiscard")
        compare(discard.text, qsTr("Descartar"))
        verify(discard.focusPolicy === Qt.NoFocus)
        mouseClick(discard)
        compare(SelenitaController.calls,
                ["toggleRecording:false", "discardRecording:recording"])
        compare(SelenitaController.recordingState, "idle")
        compare(SelenitaController.history.length, 0, "no history row")
        compare(SelenitaController.lastRecordingName, "")
        verify(!bar.visible)
    }

    // The actions are one row inside the pill, which grows with it:
    // «Pausar», «Detener» and «Descartar», in that order.
    function test_the_actions_share_one_row_the_pill_follows() {
        const bar = makeBar()
        record(bar)
        const actions = named(bar, "recordingBarActions")
        compare(actions.children.length, 3)
        compare(actions.children[0].objectName, "recordingBarPause")
        compare(actions.children[1].objectName, "recordingBarStop")
        compare(actions.children[2].objectName, "recordingBarDiscard")
        const pill = named(bar, "recordingBarPill")
        tryCompare(bar, "width", pill.implicitWidth)
        tryCompare(bar, "height", pill.implicitHeight)
        verify(pill.implicitWidth > actions.width)
    }

    // The bar's clock is the card's: from the controller's origin, reset by
    // a new one, 0 when nothing records.
    function test_the_clock_reads_from_the_start() {
        const bar = makeBar()
        const elapsed = named(bar, "recordingBarElapsed")
        SelenitaController.recordingStartedAt = Date.now() - 61500
        SelenitaController.recordingState = "recording"
        compare(elapsed.text, "1:01")
        compare(bar.elapsed, 61)
        SelenitaController.recordingStartedAt = Date.now() - 3725000
        compare(elapsed.text, "1:02:05")
        SelenitaController.recordingState = "idle"
        compare(bar.elapsed, 0)
        SelenitaController.recordingStartedAt = Date.now()
        SelenitaController.recordingState = "recording"
        compare(elapsed.text, "0:00")
        // It ticks on its own.
        tryCompare(elapsed, "text", "0:01", 2500)
    }

    // The pill keeps its width as the seconds go by.
    function test_the_width_holds_as_the_time_grows() {
        const bar = makeBar()
        SelenitaController.recordingStartedAt = Date.now() - 5000
        SelenitaController.recordingState = "recording"
        const pill = named(bar, "recordingBarPill")
        const narrow = pill.implicitWidth
        SelenitaController.recordingStartedAt = Date.now() - 3725000
        compare(pill.implicitWidth, narrow)
    }

    // A compositor's close is not a stop: refused while it shows, nothing
    // asked of the controller. Once the application is ending it goes.
    function test_a_compositor_close_is_refused() {
        const bar = makeBar()
        record(bar)
        bar.close()
        verify(bar.visible, "still on screen")
        compare(SelenitaController.recordingState, "recording")
        compare(SelenitaController.calls, ["toggleRecording:false"])
        bar.hideNow()
        verify(!bar.visible, "the quit takes it")
        compare(SelenitaController.recordingState, "recording")
    }

    function test_reduced_motion_stills_the_dot() {
        const bar = makeBar()
        record(bar)
        const dot = named(bar, "recordingBarDot")
        verify(dot.visible)
        verify(bar.pulsing, "it pulses")
        CelestinaTheme.reducedMotion = true
        // The cycle under way ends at full opacity.
        tryVerify(function() { return !bar.pulsing }, 2000)
        compare(dot.opacity, 1)
        CelestinaTheme.reducedMotion = false
        verify(bar.pulsing)
    }

    // «Pausar» pauses, the button becomes «Reanudar» and resumes; the bar
    // stays, the dot stops pulsing while paused.
    function test_pausar_and_reanudar_take_turns() {
        const bar = makeBar()
        record(bar)
        const pause = named(bar, "recordingBarPause")
        compare(pause.text, qsTr("Pausar"))
        verify(pause.focusPolicy === Qt.NoFocus)
        verify(bar.pulsing)
        mouseClick(pause)
        compare(SelenitaController.recordingState, "paused")
        verify(bar.visible, "the bar stays while paused")
        compare(pause.text, qsTr("Reanudar"))
        tryVerify(function() { return !bar.pulsing }, 2000, "the dot stands still")
        compare(named(bar, "recordingBarDot").opacity, 1)
        compare(named(bar, "recordingBarPill").Accessible.description, qsTr("En pausa"))
        mouseClick(pause)
        compare(SelenitaController.recordingState, "recording")
        compare(pause.text, qsTr("Pausar"))
        verify(bar.pulsing)
        compare(SelenitaController.calls, ["toggleRecording:false",
                                           "pauseRecording:recording",
                                           "resumeRecording:paused"])
    }

    // The clock counts recorded time only: it holds while paused and goes
    // on from there after the resume.
    function test_the_clock_stands_still_while_paused() {
        const bar = makeBar()
        const elapsed = named(bar, "recordingBarElapsed")
        SelenitaController.recordingStartedAt = Date.now() - 5200
        SelenitaController.recordingState = "recording"
        compare(elapsed.text, "0:05")
        SelenitaController.pauseRecording()
        compare(elapsed.text, "0:05")
        wait(1300)
        compare(elapsed.text, "0:05", "held while paused")
        SelenitaController.resumeRecording()
        compare(elapsed.text, "0:05", "from where it stopped")
        tryCompare(elapsed, "text", "0:06", 2500)
    }

    // The pause button keeps the wider label's width, so the pill does not
    // change size between «Pausar» and «Reanudar».
    function test_the_pill_keeps_its_width_through_a_pause() {
        const bar = makeBar()
        record(bar)
        const pill = named(bar, "recordingBarPill")
        const pause = named(bar, "recordingBarPause")
        const wide = pill.implicitWidth
        const button = pause.width
        SelenitaController.pauseRecording()
        compare(pause.text, qsTr("Reanudar"))
        compare(pause.width, button)
        compare(pill.implicitWidth, wide)
    }

    // «Detener» and «Descartar» act from paused as from recording.
    function test_detener_and_descartar_act_from_paused() {
        const bar = makeBar()
        record(bar)
        SelenitaController.pauseRecording()
        mouseClick(named(bar, "recordingBarStop"))
        compare(SelenitaController.recordingState, "idle")
        compare(SelenitaController.history.length, 1)
        verify(!bar.visible)
        record(bar)
        SelenitaController.pauseRecording()
        mouseClick(named(bar, "recordingBarDiscard"))
        compare(SelenitaController.recordingState, "idle")
        compare(SelenitaController.history.length, 1, "no row for the discard")
        verify(SelenitaController.calls.indexOf("stopRecording:paused") >= 0)
        verify(SelenitaController.calls.indexOf("discardRecording:paused") >= 0)
    }

    function test_every_action_has_a_spanish_name() {
        const bar = makeBar()
        record(bar)
        const pill = named(bar, "recordingBarPill")
        compare(pill.Accessible.name, qsTr("Grabando"))
        verify(named(bar, "recordingBarStop").Accessible.description.length > 0)
        verify(named(bar, "recordingBarPause").Accessible.description.length > 0)
        verify(named(bar, "recordingBarDiscard").Accessible.description.length > 0)
        verify(named(bar, "recordingBarElapsed").Accessible.name.length > 0)
    }
}
