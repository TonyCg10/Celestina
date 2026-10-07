import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

// A wheel notch must move the view on the next frame, land exactly one
// compWheelStep away without a tail, and a fast burst of notches must add up
// exactly. Qt's own wheel handling turns a notch into a decelerating flick:
// about 27 px on the first frame, 72 px in all, settling after ~300 ms, and a
// burst of ten notches 15 ms apart only covers about a third of ten steps.
TestCase {
    id: testCase

    name: "CelestinaWheelScroll"
    when: testWindow.visible

    Window {
        id: testWindow

        width: 400
        height: 600
        visible: true

        // Whatever the list leaves alone reaches this outer scroller.
        MouseArea {
            id: outer

            property int seen: 0

            anchors.fill: parent
            onWheel: function(wheel) {
                outer.seen++
                wheel.accepted = true
            }

            ListView {
                id: list

                property var log: []

                anchors.fill: parent
                model: 2000
                boundsBehavior: Flickable.StopAtBounds
                delegate: Rectangle {
                    required property int index
                    width: list.width
                    height: 40
                    color: index % 2 ? CelestinaTheme.surface : CelestinaTheme.surfaceStrong
                }
                onContentYChanged: list.log.push([Date.now(), list.contentY])

                CelestinaWheelScroll {
                    id: wheel
                    view: list
                }

                // A Ctrl+wheel zoom the way Grafita declares one, inside the
                // view. It does not block, so the event still reaches the
                // scroller's area, which must leave it alone. (Qt delivers
                // nothing behind a ListView's content on Ctrl+wheel, with or
                // without this scroller, so the zoom cannot sit further out.)
                WheelHandler {
                    id: zoom

                    property int seen: 0

                    blocking: false
                    acceptedModifiers: Qt.ControlModifier
                    onWheel: zoom.seen++
                }
            }
        }
    }

    SignalSpy {
        id: steppedSpy
        target: wheel
        signalName: "stepped"
    }

    readonly property real step: CelestinaTheme.compWheelStep

    function init() {
        CelestinaTheme.reducedMotion = false
        list.cancelFlick()
        list.contentY = list.originY - list.topMargin
        wait(60)
        list.log = []
        outer.seen = 0
        zoom.seen = 0
        steppedSpy.clear()
    }

    function cleanup() {
        CelestinaTheme.reducedMotion = false
    }

    function notch(dy, modifiers) {
        mouseWheel(list, list.width / 2, list.height / 2, 0, dy, Qt.NoButton,
                   modifiers === undefined ? Qt.NoModifier : modifiers)
    }

    function test_a_one_notch_moves_at_once_and_lands_exactly() {
        notch(-120)
        tryVerify(function() { return list.log.length > 0 }, 200)
        const first = list.log[0][1]
        verify(first >= step * 0.3, "first frame moved " + first + " px")
        tryCompare(list, "contentY", step, 200)
        wait(150)
        compare(list.contentY, step)
        compare(outer.seen, 0)
        // The gesture's full delta, signed like angleDelta.y, marked a notch.
        compare(steppedSpy.count, 1)
        compare(steppedSpy.signalArguments[0][0], -step)
        compare(steppedSpy.signalArguments[0][1], true)
    }

    function test_b_burst_adds_up_exactly() {
        let last = 0
        for (let i = 0; i < 10; ++i) {
            notch(-120)
            last = Date.now()
            if (i < 9)
                wait(15)
        }
        tryCompare(list, "contentY", 10 * step, 1000)
        let reached = -1
        for (const sample of list.log) {
            if (sample[1] >= 9 * step) {
                reached = sample[0] - last
                break
            }
        }
        verify(reached >= 0 && reached <= 120, "90 % reached " + reached + " ms after the last notch")
        wait(150)
        compare(list.contentY, 10 * step)
        compare(outer.seen, 0)
    }

    function test_c_bounds_pass_the_event_on() {
        notch(120)
        compare(outer.seen, 1, "an upward notch at the top goes to the outer scroller")
        wait(100)
        compare(list.contentY, list.originY - list.topMargin)

        const bottom = list.originY + list.contentHeight + list.bottomMargin - list.height
        list.contentY = bottom
        wait(60)
        outer.seen = 0
        notch(-120)
        compare(outer.seen, 1, "a downward notch at the bottom goes to the outer scroller")
        wait(100)
        compare(list.contentY, bottom)
    }

    function test_d_direct_write_wins_over_the_glide() {
        notch(-120)
        notch(-120)
        notch(-120)
        tryVerify(function() { return list.contentY > 0 }, 200)
        list.contentY = 500
        wait(250)
        compare(list.contentY, 500, "the glide stopped instead of fighting the write")
    }

    function test_e_ctrl_wheel_is_left_alone() {
        notch(-120, Qt.ControlModifier)
        compare(zoom.seen, 1, "Ctrl+wheel reached the zoom handler")
        compare(steppedSpy.count, 0, "the scroller left Ctrl+wheel alone")
    }

    function test_f_reduced_motion_jumps_one_step() {
        CelestinaTheme.reducedMotion = true
        notch(-120)
        compare(list.contentY, step)
        wait(100)
        compare(list.contentY, step)
    }

    function test_g_retarget_clamps_after_geometry_change() {
        notch(-120)
        tryCompare(list, "contentY", step, 400)
        list.model = 10
        wait(60)
        wheel.retarget()
        compare(list.contentY, list.originY - list.topMargin)
        list.model = 2000
    }
}
