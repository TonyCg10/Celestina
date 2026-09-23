import QtQuick
import org.celestina.siderita 1.0

// ─── FolderBottomStatus ─────────────────────────────────────────────────────
// Where the transient column lives. There were three surfaces here with three
// placements — two full-width bands across the rows and a strip repeating what
// the ring already said — and each one's `y` depended on whether the one before
// it was visible. It is now one column anchored to the bottom right, and its
// own height is what lifts its top away from the bar.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    required property var controller
    required property Item panel
    required property Item bottomControls
    required property Item contentFrame
    required property Item bottomBar
    required property Item bottomView

    ActivityStack {
        controller: root.controller
        backdrop: root.bottomView
        // The widest a notice may be: the frame minus the capsule and the two
        // gaps around it.
        maxNoticeWidth: Math.max(
            160,
            root.contentFrame.width - 2 * root.panel.floatingChromeInset
            - root.bottomControls.implicitWidth - 2 * CelestinaTheme.spaceMd)
        x: root.contentFrame.x + root.contentFrame.width
           - root.panel.floatingChromeInset - width
        // It grows upward because its own height decides where its top is, not
        // because the column changes which end it is anchored to. See
        // ActivityStack.
        y: root.bottomBar.y - CelestinaTheme.compFloatingGap - height
        z: 5
    }
}
