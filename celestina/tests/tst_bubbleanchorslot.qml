import QtQuick
import QtTest
import "../qml" as Desktop

// M7's exit criterion in one place: minimizing the first window must have a real
// destination before any bubble exists, and gaining bubbles must not move that
// destination. Both are layout facts, so they are settled here rather than inferred
// from a running session where the clock and the tray are also changing width.
TestCase {
    id: testCase

    name: "BubbleAnchorSlot"
    when: windowShown
    width: 800
    height: 60

    function reading(count) {
        const windows = [];
        for (let index = 0; index < count; ++index) {
            windows.push({
                "id": String(index + 1),
                "appId": "org.example.App",
                "title": "Window " + index,
                "iconName": ""
            });
        }
        return {"available": true, "revision": String(count + 1), "windows": windows};
    }

    Desktop.BackdropInk { id: testInk }

    // The panel's right flank hugs the screen edge, so the arrangement under test is a
    // trailing cluster: the group grows away from the edge and the slot holds the edge.
    Item {
        id: flank

        anchors.fill: parent

        Desktop.PanelCluster {
            id: cluster

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            barHeight: 40
            blurAvailable: false
            ink: testInk

            Desktop.BubbleGroup {
                id: group

                reading: testCase.reading(0)
                ink: testInk
            }

            Desktop.BubbleAnchorSlot {
                id: slot

                outputName: "DP-1"
            }
        }
    }

    function slotX() {
        return slot.mapToItem(flank, 0, 0).x;
    }

    function test_the_slot_exists_before_the_first_bubble_does() {
        group.reading = reading(0);
        wait(0);
        compare(group.bubbleCount, 0);
        compare(slot.width, 22);
        compare(slot.height, 22);
        verify(slotX() > 0, "a minimize with no bubbles yet still has somewhere to go");
    }

    function test_gaining_bubbles_does_not_move_the_slot() {
        group.reading = reading(0);
        wait(0);
        const empty = slotX();

        group.reading = reading(1);
        wait(0);
        compare(group.bubbleCount, 1);
        compare(slotX(), empty, "the first bubble moved the anchor");

        group.reading = reading(3);
        wait(0);
        compare(group.bubbleCount, 3);
        compare(slotX(), empty, "a growing group moved the anchor");

        // And back down again: a bubble leaving must not move it either.
        group.reading = reading(1);
        wait(0);
        compare(slotX(), empty, "a shrinking group moved the anchor");
    }

    function test_the_slot_draws_nothing_and_takes_no_input() {
        // It is a coordinate, not a control.
        compare(slot.enabled, false);
        compare(slot.Accessible.ignored, true);
        compare(slot.children.length, 0);
    }

    function test_reduced_motion_wins_over_the_anchor() {
        const reduced = slot.transitionOptions(true);
        compare(reduced.transition, "disabled");
        compare(reduced.anchor_output, undefined);

        const travelling = slot.transitionOptions(false);
        compare(travelling.transition, "anchored");
        compare(travelling.anchor_output, "DP-1");
        compare(travelling.anchor_width, 22);
        compare(travelling.anchor_height, 22);
    }
}
