import QtQuick
import QtTest
import CelestinaStyle
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
            // What a strip with nothing to collapse looks like, and what the
            // host fills in for a helper that predates grouping: every
            // workspace at home on the output it is on, every group open.
            "home": "DP-1",
            "groupExpanded": true,
            "groupFocus": false,
            "active": active,
            "focused": active,
            "urgent": false,
            "activeWindowTitle": "",
            "requestState": requestState
        };
    }

    // The author's displaced session in miniature: three monitors' workspaces
    // all arriving on the survivor, with the adapter naming the home of each
    // and opening exactly one group.
    function displaced(openHome, urgentHome) {
        const homes = ["DP-1", "HDMI-A-1", "DP-2"];
        const list = [];
        for (let slot = 0; slot < homes.length; ++slot) {
            for (let step = 1; step <= 3; ++step) {
                const index = slot * 3 + step;
                const entry = testCase.workspace(index, String(index), false, "");
                entry.home = homes[slot];
                entry.groupExpanded = homes[slot] === openHome;
                // The one a closed group's capsule asks for.
                entry.groupFocus = step === 1;
                entry.urgent = homes[slot] === urgentHome && step === 2;
                list.push(entry);
            }
        }
        return list;
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

    function test_one_monitors_workspaces_are_one_open_group() {
        compare(strip.workspaceGroups.length, 1);
        verify(!strip.grouped);
        verify(strip.workspaceGroups[0].expanded);
        compare(strip.workspaceGroups[0].workspaces.length, 3);
    }

    function test_a_displaced_strip_folds_into_one_group_per_monitor() {
        strip.workspaces = testCase.displaced("HDMI-A-1", "");

        compare(strip.workspaceGroups.length, 3);
        verify(strip.grouped);
        // The compositor's own order, so the strip does not rearrange itself
        // between frames for a reason nobody can see.
        compare(strip.workspaceGroups.map((group) => {
            return group.key;
        }), ["DP-1", "HDMI-A-1", "DP-2"]);
        compare(strip.workspaceGroups.filter((group) => {
            return group.expanded;
        }).length, 1);
        compare(strip.workspaceGroups[1].expanded, true);
    }

    function test_a_closed_group_still_reports_urgency() {
        strip.workspaces = testCase.displaced("HDMI-A-1", "DP-2");

        const closed = strip.workspaceGroups[2];
        verify(!closed.expanded);
        verify(closed.urgent);
    }

    function test_a_closed_group_names_the_workspace_it_would_ask_for() {
        strip.workspaces = testCase.displaced("HDMI-A-1", "");

        compare(strip.workspaceGroups[2].focusTarget.index, 7);
    }

    // A producer that predates grouping sends no `home`. It remains one flat
    // group rather than inventing output labels the producer never supplied.
    function test_a_helper_that_never_heard_of_homes_draws_a_flat_strip() {
        const list = testCase.board(2, -1).filter((entry) => {
            return entry.output === "DP-1";
        });
        for (let index = 0; index < list.length; ++index) {
            delete list[index].home;
            delete list[index].groupExpanded;
            delete list[index].groupFocus;
        }
        strip.workspaces = list;

        compare(strip.workspaceGroups.length, 1);
        verify(strip.workspaceGroups[0].expanded);
        verify(!strip.grouped);
    }

    function test_an_inactive_empty_singleton_spare_is_not_a_monitor_group() {
        const list = testCase.displaced("HDMI-A-1", "").filter((workspace) => {
            return workspace.home !== "DP-1";
        });
        const spare = testCase.workspace(16, "16", false, "");
        spare.home = "DP-1";
        spare.groupExpanded = false;
        list.push(spare);
        strip.workspaces = list;

        compare(strip.workspaceGroups.length, 2);
        compare(strip.workspaceGroups.map((group) => group.key),
                ["HDMI-A-1", "DP-2"]);
    }
}
