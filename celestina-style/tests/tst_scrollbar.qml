import QtQuick
import QtTest
import CelestinaStyle

// The scroll bar's position and its drag are one mapping read in two
// directions. What is asserted here is the case that broke them apart: once a
// document is long enough for `minimumHandle` to clamp the handle, the handle
// is no longer its proportional length, and any conversion that assumes it is
// disagrees with the drag — the handle reaching the end of the track with
// document still to read, and running ahead of the pointer while dragged.
//
// This is arithmetic over a Flickable's geometry, so it is observable without a
// GPU. Whether the bar reads well under the pointer at the author's scale is
// VAL-STYLE-04.
TestCase {
    id: testCase

    name: "ScrollBar"
    when: windowShown

    // 1% visible over an 800 px track: the visible fraction asks for an 8 px
    // handle and `minimumHandle` refuses, which is precisely the disagreement.
    Flickable {
        id: longDocument
        width: 40
        height: 800
        contentWidth: 40
        contentHeight: 80000
    }

    CelestinaScrollBar {
        id: bar
        surface: longDocument
        width: 8
        height: 800
    }

    Flickable {
        id: shortDocument
        width: 40
        height: 400
        contentWidth: 40
        contentHeight: 400
    }

    CelestinaScrollBar {
        id: idleBar
        surface: shortDocument
        width: 8
        height: 400
    }

    function init() {
        longDocument.contentY = 0
        wait(0)
    }

    // The premise of every case below: without the clamp there is nothing to
    // disagree about.
    function test_the_minimum_handle_clamp_is_active() {
        tryCompare(bar, "handleLength", bar.minimumHandle)
        verify(bar.shownFraction * bar.trackLength < bar.minimumHandle,
               "the visible fraction would have produced a larger handle")
        compare(bar.handleTravel, bar.trackLength - bar.minimumHandle)
    }

    // The end of the track means the end of the document, and nothing before
    // it. The defect ran the handle to the end early and pinned it there while
    // the last screens still scrolled underneath, which is the visible half of
    // the disagreement: the bar stops answering while the document moves.
    function test_the_handle_reaches_the_end_with_the_document() {
        const travel = longDocument.contentHeight - longDocument.height

        longDocument.contentY = travel * 0.99
        wait(0)
        verify(bar.handleOffset < bar.handleTravel,
               "the handle is already at the end with 1% of the document left")

        longDocument.contentY = travel
        wait(0)
        fuzzyCompare(bar.handleOffset, bar.handleTravel, 0.5)
    }

    function test_the_handle_starts_at_the_start() {
        compare(bar.handleOffset, 0)
    }

    // Position is the inverse of the drag: what `scrollToHandle` is asked for
    // is where the handle then is. A conversion factor that differs between the
    // two is exactly what makes a dragged handle drift from the pointer.
    function test_position_inverts_the_drag() {
        const offsets = [0, 1, 37, bar.handleTravel / 3, bar.handleTravel / 2,
                         bar.handleTravel - 1, bar.handleTravel]
        for (const offset of offsets) {
            bar.scrollToHandle(offset)
            wait(0)
            fuzzyCompare(bar.handleOffset, offset, 0.5,
                         "handle asked for " + offset + ", sitting at "
                         + bar.handleOffset)
        }
    }

    // Halfway down the document is halfway along what the handle can travel —
    // not halfway along the track, which is where the old conversion put it.
    function test_the_middle_of_the_document_is_the_middle_of_the_travel() {
        longDocument.contentY = (longDocument.contentHeight - longDocument.height) / 2
        wait(0)
        fuzzyCompare(bar.handleOffset, bar.handleTravel / 2, 0.5)
    }

    // Nothing to travel through: the division that converts between the two
    // distances has no denominator, and the guard is what keeps it from
    // producing one.
    function test_an_unscrollable_surface_has_no_offset() {
        compare(idleBar.contentTravel, 0)
        compare(idleBar.handleOffset, 0)
        verify(!idleBar.visible, "a surface that fits should show no bar")
        idleBar.scrollToHandle(120)
        compare(shortDocument.contentY, 0)
    }
}
