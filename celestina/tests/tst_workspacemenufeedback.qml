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

    function test_group_press_changes_the_group_mark() {
        const pointer = findChild(group, "celestina-workspace-group-pointer");
        const mark = findChild(group, "celestina-workspace-group-mark");
        verify(pointer);
        verify(mark);
        // The collapsed monitor is one dot, larger than the workspace dots
        // beside it, with no visible count or bordered capsule.
        verify(mark.width > 12);
        verify(!findChild(group, "celestina-workspace-group-feedback"));

        mousePress(group, group.width / 2, group.height / 2,
                   Qt.RightButton);
        verify(pointer.pressed);
        tryCompare(mark, "scale", 0.82);
        mouseRelease(group, group.width / 2, group.height / 2,
                     Qt.RightButton);
    }

    function test_both_controls_publish_the_attachment_source_contract() {
        verify(workspace.isPanelAttachmentSource);
        verify(group.isPanelAttachmentSource);
        verify(workspace.attachmentAnchor);
        verify(group.attachmentAnchor);
        const pillRect = workspace.attachmentAnchorGlobalRectNow();
        const groupRect = group.attachmentAnchorGlobalRectNow();
        verify(pillRect.width > 0 && pillRect.height > 0);
        verify(groupRect.width > pillRect.width);
    }
}
