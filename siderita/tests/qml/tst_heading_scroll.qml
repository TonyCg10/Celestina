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
        expandSpan: 56
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

    function test_a_travel_zero_is_the_compact_heading() {
        compare(heading.compactProgress, 1, "at rest the heading is compact")
        compare(heading.retiredProgress, 0, "and it is still there")
    }

    function test_b_negative_travel_grows_it_and_positive_takes_it_away() {
        heading.moveTo(-56, false)
        compare(heading.compactProgress, 0, "fully expanded")
        heading.moveTo(-28, false)
        compare(heading.compactProgress, 0.5, "and half way, continuously")
        heading.moveTo(120, false)
        compare(heading.retiredProgress, 1, "fully retired")
        heading.moveTo(60, false)
        compare(heading.retiredProgress, 0.5, "and half way there too")
    }

    function test_c_travel_is_clamped_to_its_two_ends() {
        heading.advance(-1000, true, false)
        compare(heading.travel, -56, "it cannot expand past its own span")
        heading.advance(1000, true, false)
        compare(heading.travel, 200, "nor past the credit above the fade")
        compare(heading.retiredProgress, 1, "which is still fully retired")
    }

    // Expanding is still something you ask for at the top. Away from it the
    // travel stops at the compact heading instead of growing it.
    function test_d_it_only_expands_at_the_top() {
        heading.moveTo(40, false)
        heading.advance(-1000, false, false)
        compare(heading.travel, 0, "it came back to compact and stopped")
        heading.advance(-1000, true, false)
        compare(heading.travel, -56, "and at the top it keeps going")
    }

    // Once the title is gone the travel keeps accumulating, and coming back up
    // spends that credit before the title starts to return. Without it a single
    // notch upward from deep inside a folder brought most of the heading back.
    function test_n_coming_back_spends_the_credit_before_the_title_returns() {
        heading.advance(1000, false, false)          // scrolled well past gone
        compare(heading.travel, 200)
        heading.advance(-80, false, false)           // the whole credit
        compare(heading.retiredProgress, 1,
                "the title came back while the credit was still unspent")
        heading.advance(-60, false, false)
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

    // Coming back up un-ramps it, and only past the top does it expand.
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
        heading.advance(30, true, false)
        compare(heading.travel, 30, "an unsmoothed advance should not animate")
        heading.advance(30, true, true)
        verify(heading.travel < 60, "a smoothed advance should not land at once")
        tryVerify(function() { return heading.travel === 60 }, 2000,
                  "and it should still arrive")
    }

    // The author's recording: scrolling up, the listing kept rising into the
    // top margin showing an empty band while the heading did nothing, and only
    // at the far end of that margin did it start to grow. The margin is meant
    // to *be* the room the metadata block grows into, so the heading starts as
    // soon as the rows themselves are fully shown — `originY`, not the margin's
    // far side.
    function test_j_expanding_begins_where_the_rows_end_not_past_the_margin() {
        view.topMargin = 56
        handler.resetTarget()
        view.contentY = view.originY      // rows fully shown; margin untouched
        mouseWheel(view, 200, 160, 0, 120)
        tryVerify(function() { return heading.travel < 0 }, 2000,
                  "the whole top margin was dead scroll before the heading moved")
        view.topMargin = 0
        handler.resetTarget()
    }

    // ── The row-slide fix: expanding pins the listing, it does not scroll it ──
    //
    // The frame the rows sit in only moves while the heading is expanding or
    // collapsing back (`travel < 0`); the rest of the time it is static. It
    // used to move at the wheel's raw rate on `contentY` while the frame
    // itself moved at the much slower rate the heading's own span sets — two
    // different speeds on the same pixels, which is what the author saw as
    // the rows sliding.

    // There is nothing above row 0 to reveal while the heading grows, so the
    // whole notch goes to the heading and the listing does not move at all.
    function test_p_expanding_does_not_move_the_listing() {
        // The resting margin already carries the room the metadata block
        // grows into — production sets it before any gesture starts, so the
        // fixture must too, or `boundedY` alone would hide the bug by
        // clamping the notch back to origin for an unrelated reason.
        view.topMargin = 60
        handler.resetTarget()
        mouseWheel(view, 200, 160, 0, 120)      // upward, at rest and at top
        tryVerify(function() { return heading.travel < 0 }, 2000,
                  "the heading did not start expanding")
        compare(view.contentY, view.originY,
                "the listing moved while there was nothing above it to reveal")
        mouseWheel(view, 200, 160, 0, 120)      // a second notch, still expanding
        tryVerify(function() { return heading.travel <= -1 }, 2000)
        compare(view.contentY, view.originY,
                "a second notch also should not have moved the listing")
        view.topMargin = 0
    }

    // Collapsing back is the same trade the other way: the notch that folds
    // the heading does not also scroll, or the two would fight over the same
    // pixels — exactly the mismatch that produced the slide.
    function test_q_collapsing_back_does_not_scroll_either() {
        heading.moveTo(-56, false)              // fully expanded, at rest
        mouseWheel(view, 200, 160, 0, -20)       // downward, collapsing it a little
        tryVerify(function() { return heading.travel > -56 }, 2000,
                  "the heading did not start collapsing")
        verify(heading.travel < 0, "one small notch should not finish collapsing it")
        compare(view.contentY, view.originY,
                "the listing scrolled while the heading was still collapsing")
    }

    // The frame keeps moving for the whole span of the glide, long after the
    // wheel event that started it has returned. Simulated here as the real
    // chain would drive it: pinned to the origin on every step, not just the
    // one the wheel touched.
    function test_r_the_listing_tracks_a_moving_origin_while_expanding() {
        view.topMargin = 60
        handler.resetTarget()
        mouseWheel(view, 200, 160, 0, 120)         // start expanding
        tryVerify(function() { return heading.travel < 0 }, 2000)
        view.topMargin = 20      // the frame rose, as it does in the real chain
        compare(view.contentY, view.originY,
                "the listing did not follow the origin the frame's own rise moved")
        view.topMargin = 45
        compare(view.contentY, view.originY,
                "nor on the next frame of that same rise")
        view.topMargin = 0
    }

    // Once it is back to fully compact, scrolling resumes exactly where the
    // pin left it — no jump, because the target the wheel accumulates into is
    // re-read from the actual position the first time it is needed again.
    function test_s_scrolling_resumes_with_no_jump_once_compact_again() {
        // A little collapsing left to do, with the margin already where the
        // real chain would have put it for that amount of travel — the pin
        // handler above holds the listing there, the same way it would mid-
        // glide in production.
        heading.moveTo(-10, false)
        view.topMargin = 50
        handler.resetTarget()
        compare(view.contentY, view.originY,
                "the pin did not hold right before the collapsing notch")
        const pinnedAt = view.contentY

        mouseWheel(view, 200, 160, 0, -60)   // enough to finish collapsing
        tryVerify(function() { return heading.travel >= 0 }, 2000,
                  "it never finished collapsing")
        // The instant it lands there is nothing above row 0 left to reveal
        // yet — the notch that crosses back is still spent on the heading —
        // so the listing must still be exactly where the pin left it.
        verify(Math.abs(view.contentY - pinnedAt) < 2,
               "the listing jumped the moment the heading finished collapsing")

        mouseWheel(view, 200, 160, 0, -120)   // an ordinary scroll now
        tryVerify(function() { return view.contentY > pinnedAt + 5 }, 2000,
                  "scrolling did not resume after the handoff")
    }

    function test_g_coming_back_up_restores_then_expands() {
        heading.moveTo(120, false)
        view.contentY = 600
        mouseWheel(view, 200, 160, 0, 120)
        tryVerify(function() { return heading.travel < 120 }, 2000,
                  "it did not come back")
        verify(heading.travel >= 0, "it expanded before reaching the top")
        view.contentY = 0
        handler.resetTarget()
        heading.moveTo(0, false)
        mouseWheel(view, 200, 160, 0, 120)
        tryVerify(function() { return heading.travel < 0 }, 2000,
                  "at the top a push up expands it")
    }
}
