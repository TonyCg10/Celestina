import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The heading is one number now, not three states.
//
// It used to have a detent per transition, and each one swallowed the gesture
// that crossed it: the notch that folded the heading did not scroll the list,
// and neither did the one that took the title away. That is the double scroll
// the author felt. Travel now moves the heading and the listing together, and
// the heading's progress is a continuous function of that travel. The travel
// has no room above zero any more: the metadata block that grew on a push up
// at the top is gone, so that push is nothing.
TestCase {
    id: testCase
    name: "HeadingScroll"
    width: 420
    height: 320
    visible: true
    when: windowShown

    HeadingScroll {
        id: heading
        retireSpan: 120
        returnDelay: 80
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
        heading.moveTo(0, false)
        view.topMargin = 0
        view.contentY = 0
        handler.resetTarget()
    }


    // ── The mapping, with no view in the way ──────────────────────────

    function test_a_travel_zero_is_the_heading_at_rest() {
        compare(heading.retiredProgress, 0, "at rest the heading is there")
    }

    function test_b_positive_travel_takes_it_away() {
        heading.moveTo(120, false)
        compare(heading.retiredProgress, 1, "fully retired")
        heading.moveTo(60, false)
        compare(heading.retiredProgress, 0.5, "and half way there, continuously")
    }

    function test_c_travel_is_clamped_to_its_two_ends() {
        heading.advance(-1000, false)
        compare(heading.travel, 0, "it cannot go above the title at rest")
        heading.advance(1000, false)
        compare(heading.travel, 200, "nor past the credit above the fade")
        compare(heading.retiredProgress, 1, "which is still fully retired")
    }

    // Once the title is gone the travel keeps accumulating, and coming back up
    // spends that credit before the title starts to return. Without it a single
    // notch upward from deep inside a folder brought most of the heading back.
    function test_n_coming_back_spends_the_credit_before_the_title_returns() {
        heading.advance(1000, false)                 // scrolled well past gone
        compare(heading.travel, 200)
        heading.advance(-80, false)                  // the whole credit
        compare(heading.retiredProgress, 1,
                "the title came back while the credit was still unspent")
        heading.advance(-60, false)
        compare(heading.retiredProgress, 0.5,
                "and only then does it return, continuously")
    }

    // ── The gesture, through the real wheel handler ───────────────────

    function test_e_one_notch_moves_the_heading_and_the_listing() {
        const before = view.contentY
        mouseWheel(view, 200, 160, 0, -120)
        // Both are tweened over the same duration, which is the point: they
        // arrive together instead of one snapping while the other glides.
        tryVerify(function() { return heading.travel > 0 }, 2000,
                  "the heading did not move")
        tryVerify(function() { return view.contentY > before }, 2000,
                  "the listing did not scroll: this is the swallowed gesture")
    }

    // The old handler consumed the notch that crossed a detent. Four notches
    // down must leave the heading gone *and* the listing four notches lower.
    function test_f_scrolling_down_retires_it_without_losing_a_notch() {
        for (let index = 0; index < 4; index++)
            mouseWheel(view, 200, 160, 0, -120)
        tryVerify(function() { return heading.retiredProgress === 1 }, 2000,
                  "the title never went away")
        tryVerify(function() { return view.contentY > 200 }, 2000,
                  "distance was lost to the detents")
    }

    // Coming back up un-ramps it.
    // The author tested 1.6.0 and found the heading changing in hard steps.
    // A notch of a wheel is a jump, and the listing it scrolls is tweened over
    // `motionNormal` — so the heading has to be tweened over the same duration
    // or the two arrive at different times, which is what reads as a snap.
    function test_h_a_wheel_notch_tweens_the_heading_as_it_tweens_the_listing() {
        mouseWheel(view, 200, 160, 0, -120)
        verify(heading.travel < heading.retireSpan,
               "the notch put the heading at its destination at once")
        const started = heading.travel
        tryVerify(function() { return heading.travel > started }, 2000,
                  "the heading never arrived")
    }

    // The title coming back moves the top bar, which moves the content frame,
    // which changes the list's top margin — every frame of the return. If that
    // cancels the gesture in flight the scroll stops dead while the heading
    // animates, which is what the author felt. A geometry change that leaves
    // the destination legal must not touch the gesture at all.
    function test_o_geometry_moving_does_not_stop_the_gesture() {
        mouseWheel(view, 200, 160, 0, -120)
        const asked = handler.targetContentY
        verify(asked > 0, "the notch asked for nothing")
        for (let step = 0; step < 6; step++)
            view.topMargin = 10 * step        // the margin growing, frame by frame
        tryVerify(function() { return Math.abs(view.contentY - asked) < 1 },
                  2000, "the scroll stopped when the geometry moved under it")
        view.topMargin = 0
    }

    // A touchpad already delivers a smooth stream of pixel deltas, and the
    // handler applies those straight through; tweening them would put the
    // heading behind the finger. `mouseWheel` cannot carry a pixel delta, so
    // the two treatments are asserted on the contract the handler calls.
    function test_i_an_unsmoothed_advance_lands_at_once() {
        heading.advance(30, false)
        compare(heading.travel, 30, "an unsmoothed advance should not animate")
        heading.advance(30, true)
        verify(heading.travel < 60, "a smoothed advance should not land at once")
        tryVerify(function() { return heading.travel === 60 }, 2000,
                  "and it should still arrive")
    }

    function test_g_coming_back_up_restores_it() {
        heading.moveTo(120, false)
        view.contentY = 600
        mouseWheel(view, 200, 160, 0, 120)
        tryVerify(function() { return heading.travel < 120 }, 2000,
                  "it did not come back")
        verify(heading.travel >= 0, "it went above the title at rest")
    }

    // A push up at the top is nothing: there is no metadata block to grow any
    // more, so neither the heading nor the listing moves. The frame under the
    // rows no longer moves with a gesture, which is what a fast scroll jumped on.
    function test_t_a_push_up_at_the_top_does_nothing() {
        view.topMargin = 60
        view.contentY = handler.minimumY()      // the top: rows below the chrome
        handler.resetTarget()
        const restingY = view.contentY
        mouseWheel(view, 200, 160, 0, 120)
        wait(300)
        compare(heading.travel, 0, "the heading moved on a push up at the top")
        compare(heading.destination, 0, "or was asked to")
        compare(view.contentY, restingY, "the listing moved past its origin")
        view.topMargin = 0
    }
}
