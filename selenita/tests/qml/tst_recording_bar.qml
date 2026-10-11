import QtQuick
import QtTest 1.3
import org.celestina.selenita 1.0

// The recording bar over the stand-in controller: it shows only while a
// recording is under way, it is the frameless, unfocused window niri's rule
// matches by its title, «Detener» and «Descartar» ask the controller, its
// clock is the recording card's, a compositor's close is refused while it
// shows, and reduced motion stills the dot.
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

    // The actions are one row inside the pill, which grows with it: the
    // next unit's pause button joins the row.
    function test_the_actions_share_one_row_the_pill_follows() {
        const bar = makeBar()
        record(bar)
        const actions = named(bar, "recordingBarActions")
        compare(actions.children.length, 2)
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

    function test_every_action_has_a_spanish_name() {
        const bar = makeBar()
        record(bar)
        const pill = named(bar, "recordingBarPill")
        compare(pill.Accessible.name, qsTr("Grabando"))
        verify(named(bar, "recordingBarStop").Accessible.description.length > 0)
        verify(named(bar, "recordingBarDiscard").Accessible.description.length > 0)
        verify(named(bar, "recordingBarElapsed").Accessible.name.length > 0)
    }
}
