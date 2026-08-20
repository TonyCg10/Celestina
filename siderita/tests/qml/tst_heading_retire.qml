import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The heading has three states and each transition has its own gesture.
//
// The expanded heading yields to any downward scroll — that is long-standing
// behaviour, and breaking it left the metadata block on screen while the rows
// slid underneath it. Taking away the *compact* heading is the deliberate one,
// and it waits for real travel. Coming back only asks for arriving at the top.
TestCase {
    id: testCase
    name: "HeadingRetire"
    width: 420
    height: 320
    visible: true
    when: windowShown

    // What the view believes the heading is doing, as FolderView hands down.
    property bool expanded: false
    property int reveals: 0
    property int restores: 0
    property int collapses: 0
    property int retires: 0

    Flickable {
        id: view
        anchors.fill: parent
        contentHeight: 4000
        contentWidth: width

        FolderWheelHandler {
            id: handler
            view: view
            headingExpanded: testCase.expanded
            onRevealRequested: testCase.reveals++
            onRestoreRequested: testCase.restores++
            onCollapseRequested: testCase.collapses++
            onRetireRequested: testCase.retires++
        }
    }

    function init() {
        testCase.expanded = false
        testCase.reveals = 0
        testCase.restores = 0
        testCase.collapses = 0
        testCase.retires = 0
        handler.retireTravel = 0
        view.contentY = 0
    }

    // One notch of a wheel is past the collapse threshold, so the expanded
    // heading folds — but it is nowhere near the retire one, so the compact
    // title stays.
    function test_a_one_notch_collapses_but_does_not_retire() {
        testCase.expanded = true
        mouseWheel(view, 200, 160, 0, -120)
        compare(testCase.collapses, 1, "one notch should fold the big heading")
        compare(testCase.retires, 0, "one notch must not take the title away")
    }

    // A touchpad easing into the list moves it without rearranging the window.
    function test_aa_a_few_pixels_do_not_fold_the_heading() {
        testCase.expanded = true
        mouseWheel(view, 200, 160, 0, 0, Qt.NoModifier, false, -18)
        compare(testCase.collapses, 0, "the heading folded on a nudge")
    }

    // Enough notches to pass the threshold: it retires, and exactly once.
    function test_b_enough_travel_retires_the_compact_heading() {
        for (let i = 0; i < 4; i++)
            mouseWheel(view, 200, 160, 0, -120)
        verify(testCase.retires > 0, "the title never went away")
        verify(testCase.retires < 4, "it went away once per notch: retires="
                                     + testCase.retires)
    }

    // Arriving at the top restores the compact heading; expanding it is the
    // separate, armed gesture.
    function test_c_reaching_the_top_restores_the_compact_heading() {
        view.contentY = 400
        mouseWheel(view, 200, 160, 0, 120)   // still on the way up
        compare(testCase.restores, 0)
        view.contentY = 0
        mouseWheel(view, 200, 160, 0, 120)   // arrived
        compare(testCase.restores, 1)
    }

    // Travel downwards does not accumulate across an upward change of mind.
    // A gesture does one thing. Collapsing the expanded heading already lifts
    // the whole chrome, and letting the same notch scroll as well is what read
    // as the content jumping away and needing a second scroll to catch up.
    function test_ba_the_gesture_that_collapses_does_not_also_scroll() {
        testCase.expanded = true
        view.contentY = 300
        const before = view.contentY
        mouseWheel(view, 200, 160, 0, -120)
        compare(testCase.collapses, 1)
        compare(view.contentY, before, "the listing scrolled as well")
    }

    // Once the heading is compact, the wheel scrolls normally again. A notch is
    // tweened rather than applied at once, so the assertion waits for it.
    function test_bb_afterwards_the_wheel_scrolls_as_usual() {
        testCase.expanded = false
        view.contentY = 300
        mouseWheel(view, 200, 160, 0, -120)
        tryVerify(function() { return view.contentY > 300 }, 2000,
                  "the listing stopped scrolling")
    }

    function test_d_changing_direction_forgets_the_travel() {
        mouseWheel(view, 200, 160, 0, -120)
        compare(testCase.retires, 0)
        mouseWheel(view, 200, 160, 0, 120)
        compare(handler.retireTravel, 0)
        mouseWheel(view, 200, 160, 0, -120)
        compare(testCase.retires, 0, "travel survived a change of direction")
    }
}
