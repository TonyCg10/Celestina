import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

// Right-click is a menu action on both workspace controls. Their primary
// actions remain unchanged, but the physical press must be visible whichever
// button will be released.
TestCase {
    id: testCase

    name: "WorkspaceMenuFeedback"
    when: windowShown
    visible: true
    width: 160
    height: 80

    Desktop.BackdropInk {
        id: testInk
    }

    Row {
        anchors.centerIn: parent
        spacing: CelestinaTheme.spaceLg

        Desktop.WorkspacePill {
            id: workspace

            ink: testInk

            workspace: ({
                "index": 1,
                "label": "1",
                "output": "DP-1",
                "active": false,
                "focused": false,
                "urgent": false,
                "requestState": "",
                "activeWindowTitle": "Terminal"
            })
        }

        Desktop.WorkspaceGroupCapsule {
            id: group

            ink: testInk
            outputName: "DP-2"
            count: 5
            urgent: false
        }
    }

    function test_workspace_press_changes_the_state_mark() {
        const pointer = findChild(workspace, "celestina-workspace-pointer");
        const mark = findChild(workspace, "celestina-workspace-state");
        verify(pointer);
        verify(mark);

        mousePress(workspace, workspace.width / 2, workspace.height / 2,
                   Qt.RightButton);
        verify(pointer.pressed);
        tryCompare(mark, "scale", 0.82);
        mouseRelease(workspace, workspace.width / 2, workspace.height / 2,
                     Qt.RightButton);
    }

    function test_group_press_uses_the_pressed_surface() {
        const pointer = findChild(group, "celestina-workspace-group-pointer");
        const feedback = findChild(group, "celestina-workspace-group-feedback");
        verify(pointer);
        verify(feedback);

        mousePress(group, group.width / 2, group.height / 2,
                   Qt.RightButton);
        verify(pointer.pressed);
        tryCompare(feedback, "color", CelestinaTheme.surfaceStrong);
        mouseRelease(group, group.width / 2, group.height / 2,
                     Qt.RightButton);
    }
}
