import QtQuick

// ─── CelestinaRowHighlight ────────────────────────────────────────────────────
// The fill behind a list row, grid cell or column title: hover, press,
// selection and drag, in the one recipe every list in the suite paints. The
// host reports the states it knows — from its MouseArea, its model, its drag —
// and this paints them; it holds no pointer of its own, so it never competes
// with the row's handlers for the click.
//
// Press is part of the contract, not an option: a row that only lights under
// the pointer and never darkens under the finger gives no sign the click was
// taken, which is where "did it register?" comes from. Ten hand-rolled copies
// of this rectangle disagreed on exactly that.
// ──────────────────────────────────────────────────────────────────────────────
Rectangle {
    id: highlight

    property bool hovered: false
    property bool pressed: false
    property bool selected: false
    property bool dragging: false
    property bool focused: false
    // Rows and cells select in two washes: the row's accent badge, the cell's
    // quieter surface. The host picks; the rest of the recipe is shared.
    property color selectedFill: CelestinaTheme.badgeAccentFill

    radius: CelestinaTheme.radiusSm
    color: highlight.dragging || highlight.pressed
           ? CelestinaTheme.surfaceStrong
           : highlight.selected
             ? highlight.selectedFill
             : highlight.hovered
               ? CelestinaTheme.surfaceHover
               : CelestinaTheme.clear
    border.width: highlight.focused ? CelestinaTheme.borderFocus : 0
    border.color: CelestinaTheme.focusRing

    Behavior on color {
        ColorAnimation {
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionFast
        }
    }
}
