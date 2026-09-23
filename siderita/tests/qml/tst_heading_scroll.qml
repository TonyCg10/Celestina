import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The heading is one number now, not three states.
//
// It used to have a detent per transition, and each one swallowed the gesture
// that crossed it: the notch that folded the heading did not scroll the list,
// and neither did the one that took the title away. That is the double scroll
// the author felt. Travel now moves the heading and the listing together, and
// the heading's two progresses are a continuous function of that travel.
TestCase {
    id: testCase
    name: "HeadingScroll"
    width: 420
    height: 320
    visible: true
    when: windowShown

    HeadingScroll {
        id: heading
        expandedExtra: 56
        retireSpan: 120
    }

    Flickable {
        id: view
        anchors.fill: parent
        contentHeight: 4000
        contentWidth: width

        FolderWheelHandler {
            id: handler
            view: view
            heading: heading
        }
    }

    function init() {
        heading.travel = 0
        view.contentY = 0
        handler.resetTarget()
    }

    // ── The mapping, with no view in the way ──────────────────────────

    function test_a_travel_zero_is_the_compact_heading() {
        compare(heading.compactProgress, 1, "at rest the heading is compact")
        compare(heading.retiredProgress, 0, "and it is still there")
    }

    function test_b_negative_travel_grows_it_and_positive_takes_it_away() {
        heading.travel = -56
        compare(heading.compactProgress, 0, "fully expanded")
        heading.travel = -28
        compare(heading.compactProgress, 0.5, "and half way, continuously")
        heading.travel = 120
        compare(heading.retiredProgress, 1, "fully retired")
        heading.travel = 60
        compare(heading.retiredProgress, 0.5, "and half way there too")
    }

    function test_c_travel_is_clamped_to_its_two_ends() {
        heading.advance(-1000, true)
        compare(heading.travel, -56, "it cannot expand past its own height")
        heading.advance(1000, true)
        compare(heading.travel, 120, "nor retire past gone")
    }

    // Expanding is still something you ask for at the top. Away from it the
    // travel stops at the compact heading instead of growing it.
    function test_d_it_only_expands_at_the_top() {
        heading.travel = 40
        heading.advance(-1000, false)
        compare(heading.travel, 0, "it came back to compact and stopped")
        heading.advance(-1000, true)
        compare(heading.travel, -56, "and at the top it keeps going")
    }

    // ── The gesture, through the real wheel handler ───────────────────

    function test_e_one_notch_moves_the_heading_and_the_listing() {
        const before = view.contentY
        mouseWheel(view, 200, 160, 0, -120)
        verify(heading.travel > 0, "the heading did not move")
        tryVerify(function() { return view.contentY > before }, 2000,
                  "the listing did not scroll: this is the swallowed gesture")
    }

    // The old handler consumed the notch that crossed a detent. Four notches
    // down must leave the heading gone *and* the listing four notches lower.
    function test_f_scrolling_down_retires_it_without_losing_a_notch() {
        for (let index = 0; index < 4; index++)
            mouseWheel(view, 200, 160, 0, -120)
        compare(heading.retiredProgress, 1, "the title never went away")
        tryVerify(function() { return view.contentY > 200 }, 2000,
                  "distance was lost to the detents")
    }

    // Coming back up un-ramps it, and only past the top does it expand.
    function test_g_coming_back_up_restores_then_expands() {
        heading.travel = 120
        view.contentY = 600
        mouseWheel(view, 200, 160, 0, 120)
        verify(heading.travel < 120, "it did not come back")
        verify(heading.travel >= 0, "it expanded before reaching the top")
        view.contentY = 0
        handler.resetTarget()
        heading.travel = 0
        mouseWheel(view, 200, 160, 0, 120)
        verify(heading.travel < 0, "at the top a push up expands it")
    }
}
