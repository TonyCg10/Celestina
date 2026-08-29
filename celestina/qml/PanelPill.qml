// SIMPLE-1. The one thing the bar paints: a floating mica capsule.
//
// The strip itself paints nothing at all — no veil, no scrim, no shadow. Each
// reading sits on this capsule and the wallpaper runs clean between them.
//
// It is a *background*, not a wrapper: it paints behind the widget it is
// placed in and takes no part in the layout, so no flank changes width and
// nothing on the bar moves. It grows only into the gap the row already had.
// Deliberately tight — a bar of fat capsules is a bar that has grown a second
// chrome.
//
// The capsule FLOATS: it is exactly one control tall, centred on its reading.
// It used to be welded to the screen's top edge — grown upward past its own
// row and squared off where it met the edge — and that is what the author
// photographed as a bug: the flank clips to the row's height, so the extra
// rise was sliced off and the capsule's sides ended in two hard vertical cuts
// with a flat lid between them. A capsule that never leaves its row cannot be
// clipped by it.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: pill

    required property BackdropInk ink
    required property bool blurAvailable
    // Panel readings use a small flank overhang; menu fields own their complete
    // rounded region and do not use this panel-specific extension. It stays
    // well inside the row's own spacing, so two neighbouring capsules never
    // touch — overlapping translucent tints would double up into a seam.
    property int horizontalOverhang: CelestinaTheme.spaceSm
    // Kept as inert API: the host and several harnesses still set it, and the
    // capsule no longer derives anything from the bar's height.
    property real barHeight: 0

    readonly property real restingWidth:
            (parent ? parent.width : 0) + pill.horizontalOverhang * 2
    readonly property real restingHeight: CelestinaTheme.controlHeightXs
    readonly property real restingX: -pill.horizontalOverhang
    readonly property real restingY:
            parent ? (parent.height - CelestinaTheme.controlHeightXs) / 2 : 0
    // Behind the reading it belongs to. A child is drawn above its parent's own
    // content unless it says otherwise, and this one is a floor.
    anchors.horizontalCenter: parent ? parent.horizontalCenter : undefined
    z: -1
    y: pill.restingY
    width: pill.restingWidth
    height: pill.restingHeight

    // The capsule is a ShellPanel like every surface (DRAWING.md): same
    // glass, same tint, same hairline — pill radius, and no shadow, because
    // the author removed every strip-level shade from the bar.
    ShellPanel {
        anchors.fill: parent
        visible: pill.visible
        radius: CelestinaTheme.radiusPill
        castsShadow: false
    }
}
