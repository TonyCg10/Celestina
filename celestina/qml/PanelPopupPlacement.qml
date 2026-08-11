// One placement rule for every surface opened from a panel control.
//
// The opener is already the real global rectangle of the control. The host
// translates it into output coordinates; this object centres the carried card
// on that rectangle, leaves only the floating-control gap below it and clamps
// the complete visible body to the output. A keybind has no opener and uses
// the caller's fallback position instead.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

QtObject {
    id: root

    required property real surfaceWidth
    required property real surfaceHeight
    required property real contentWidth
    required property real contentHeight

    property bool anchoredFromPanel: false
    property rect openerRect: Qt.rect(0, 0, 0, 0)
    property real fallbackX: (surfaceWidth - contentWidth) / 2
    property real fallbackY: (surfaceHeight - contentHeight) / 2
    property real edgeInset: 0
    property int anchorGap: CelestinaTheme.compFloatingGap
    // A model-backed panel menu may change height while it is mapped. In that
    // case the opener-relative top is the stable spatial reference and overflow
    // belongs to a bounded viewport; it must not make the whole card climb over
    // the control that opened it. Other cards retain the established whole-card
    // clamp by leaving this opt-in false.
    property bool preserveRequestedTop: false

    readonly property real desiredX: root.anchoredFromPanel
                                         ? root.openerRect.x
                                           + root.openerRect.width / 2
                                           - root.contentWidth / 2
                                         : root.fallbackX
    readonly property real desiredY: root.anchoredFromPanel
                                         ? root.openerRect.y
                                           + root.openerRect.height
                                           + root.anchorGap
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
    readonly property real y: root.preserveRequestedTop
                                   ? root.clampStart(root.desiredY,
                                                     root.surfaceHeight,
                                                     root.edgeInset)
                                   : root.clampAxis(root.desiredY,
                                                    root.surfaceHeight,
                                                    root.contentHeight,
                                                    root.edgeInset)
}
