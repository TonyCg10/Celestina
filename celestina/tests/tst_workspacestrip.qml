import QtQuick
import QtTest
import "../qml" as Desktop

// The strip's gesture arithmetic: where a step starts from, and where it lands.
// It never focuses anything here — a step is a request, and this checks only
// which workspace is asked for.
TestCase {
    id: testCase

    name: "WorkspaceStrip"

    function workspace(index, label, active, requestState) {
        return {
            "index": index,
            "label": label,
            "output": "DP-1",
            "active": active,
            "focused": active,
            "urgent": false,
            "activeWindowTitle": "",
            "requestState": requestState
        };
    }

    // Three on this output, and one on another that must never take part.
    function board(activeIndex, pendingIndex) {
        const list = [];
        for (let index = 1; index <= 3; ++index) {
            list.push(testCase.workspace(index, String(index), index === activeIndex,
                                         index === pendingIndex ? "pending" : ""));
        }
        const foreign = testCase.workspace(9, "9", true, "");
        foreign.output = "DP-2";
        list.push(foreign);
        return list;
    }

    Desktop.WorkspaceStrip {
        id: strip

        niriAvailable: true
        outputName: "DP-1"
        workspaces: testCase.board(2, -1)
    }

    SignalSpy {
        id: requests

        target: strip
        signalName: "focusRequested"
    }

    function init() {
        strip.workspaces = testCase.board(2, -1);
        requests.clear();
    }

    function test_a_step_moves_from_the_active_workspace() {
        strip.step(1);
        compare(requests.count, 1);
        compare(requests.signalArguments[0][0], "DP-1");
        compare(requests.signalArguments[0][1], 3);

        requests.clear();
        strip.step(-1);
        compare(requests.signalArguments[0][1], 1);
    }

    function test_a_step_wraps_at_both_ends() {
        strip.workspaces = testCase.board(3, -1);
        strip.step(1);
        compare(requests.signalArguments[0][1], 1);

        requests.clear();
        strip.workspaces = testCase.board(1, -1);
        strip.step(-1);
        compare(requests.signalArguments[0][1], 3);
    }

    function test_a_burst_advances_from_what_was_already_asked_for() {
        // A request is in flight for 3 while the compositor still reports 2
        // active. The next step builds on the request, not on the stale state,
        // so scrolling twice quickly moves two workspaces.
        strip.workspaces = testCase.board(2, 3);
        strip.step(1);
        compare(requests.signalArguments[0][1], 1);
    }

    function test_another_output_never_takes_part() {
        compare(strip.outputWorkspaces.length, 3);
        strip.step(1);
        compare(requests.signalArguments[0][0], "DP-1");
    }

    function test_an_output_with_no_workspaces_asks_for_nothing() {
        strip.workspaces = [];
        strip.step(1);
        compare(requests.count, 0);
    }
}
