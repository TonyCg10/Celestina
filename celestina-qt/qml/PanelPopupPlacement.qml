// One placement rule for every surface opened from a panel control.
//
// The opener is already the real global rectangle of the control. The host
// translates it into output coordinates; this object centres the carried card
// on that rectangle and leaves the semantic connector gap below the continuous
// bar. An attached body never climbs back over that bar to satisfy a bottom
// clamp: the output clips any unsupported low-resolution overflow instead of
// letting two Wayland surfaces blur the same rows. A keybind has no opener and
// uses the caller's fallback position instead.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import "EdgeAttachedGeometry.js" as EdgeAttachedGeometry

QtObject {
    id: root

    required property real surfaceWidth
    required property real surfaceHeight
    required property real contentWidth
    required property real contentHeight

    property bool anchoredFromPanel: false
    property rect openerRect: Qt.rect(0, 0, 0, 0)
    // The exact icon inside the invoking control. Placement deliberately
    // continues to follow openerRect; this independent rectangle exists only
    // for the transparent edge membrane. Missing icon geometry degrades to a
    // floating body instead of guessing the waist from the complete control.
    property rect attachmentAnchorRect: Qt.rect(0, 0, 0, 0)
    // Output-local lower edge of the continuous panel backdrop. A negative
    // value keeps the historical opener-relative placement for non-panel or
    // compatibility routes.
    property real attachmentStartY: -1
    property real fallbackX: (surfaceWidth - contentWidth) / 2
    property real fallbackY: (surfaceHeight - contentHeight) / 2
    property real edgeInset: 0
    readonly property bool edgeAttached: root.anchoredFromPanel
                                         && root.attachmentStartY >= 0
    // A floating route keeps the compact historical gap. A panel-attached
    // surface measures the connector from the bar edge, not from whichever
    // icon height happened to open it.
    property int anchorGap: root.edgeAttached
            ? Math.round(EdgeAttachedGeometry.proportionalMetric(
                  root.contentWidth,
                  CelestinaTheme.compEdgeAttachmentGapRatio,
                  CelestinaTheme.compEdgeAttachmentGapMin,
                  CelestinaTheme.compEdgeAttachmentGapMax))
            : CelestinaTheme.compFloatingGap
    // A model-backed floating menu may also choose a stable requested top while
    // it changes height. Edge-attached cards already preserve that top by
    // contract so they can never climb over the bar; this remains the explicit
    // opt-in for any other route with a bounded viewport.
    property bool preserveRequestedTop: false

    readonly property real desiredX: root.anchoredFromPanel
                                         ? root.openerRect.x
                                           + root.openerRect.width / 2
                                           - root.contentWidth / 2
                                         : root.fallbackX
    readonly property real desiredY: root.anchoredFromPanel
                                         ? (root.attachmentStartY >= 0
                                            ? root.attachmentStartY
                                              + root.anchorGap
                                            : root.openerRect.y
                                              + root.openerRect.height
                                              + root.anchorGap)
                                         : root.fallbackY

    function clampAxis(desired, available, content, inset) {
        // When a card is larger than the available span there is no pair of
        // legal edges. Centre it instead of choosing one arbitrary overflow.
        if (content + inset * 2 > available)
            return (available - content) / 2;

        return Math.max(inset, Math.min(desired,
                                        available - content - inset));
    }

    function clampStart(desired, available, inset) {
        return Math.max(inset, Math.min(desired,
                                        Math.max(inset, available - inset)));
    }

    readonly property real x: root.clampAxis(root.desiredX,
                                              root.surfaceWidth,
                                              root.contentWidth,
                                              root.edgeInset)
    readonly property real y: root.edgeAttached
                                   || root.preserveRequestedTop
                                   ? root.clampStart(root.desiredY,
                                                     root.surfaceHeight,
                                                     root.edgeInset)
                                   : root.clampAxis(root.desiredY,
                                                    root.surfaceHeight,
                                                    root.contentHeight,
                                                    root.edgeInset)
}
