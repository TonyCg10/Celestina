// PANEL-1. The Velo visual carrier for a custom overlay lifecycle.
//
// `SoftMenu` adapts the same field to a real Qt Quick `Menu`. Overlays keep
// their own Window, focus and dismissal semantics and place this card inside
// them. The card owns only presentation: one compositor-glass region, one
// reveal motion and the input stop that prevents an inside press from falling
// through to an overlay's outside-dismiss layer.
pragma ComponentBehavior: Bound

import QtQuick

SoftMenuField {
    id: root

    required property string accessibleName

    Accessible.role: Accessible.Dialog
    Accessible.name: root.accessibleName

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
    }
}
