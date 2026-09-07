import QtQuick

// ─── CelestinaRowHighlight ────────────────────────────────────────────────────
// The plate behind a list row, grid cell, tab or column title: hover, press,
// selection and drag, in the one recipe every list in the suite paints. The
// host reports the states it knows — from its MouseArea, its model, its drag —
// and this paints them; it holds no pointer of its own, so it never competes
// with the row's handlers for the click.
//
// Two families, because two kinds of thing are clicked:
//
// - `Control` — sidebar rows, tabs, column titles, menu-like rows. They behave
//   like buttons: a neutral lift under the pointer, and the press sinks.
// - `Content` — the files, cards and choices in the middle of the window. The
//   pointer already announces "this is the thing you are looking at", so the
//   hover is the accent's own hue at its faintest, the press deepens the same
//   hue, and selection settles between the two. One colour, three depths: the
//   grey plate that turned blue on the click was the transition that read as
//   a flicker.
//
// Press is part of the contract in both: the plate sinks by `rowRecoilScale`
// and takes the pressed wash, so the click is seen the instant it is taken.
// Hosts that want the content to sink with the plate bind their own scale to
// `recoil`.
// ──────────────────────────────────────────────────────────────────────────────
Rectangle {
    id: highlight

    enum Family {
        Control,
        Content
    }

    property int family: CelestinaRowHighlight.Control
    property bool hovered: false
    property bool pressed: false
    property bool selected: false
    property bool dragging: false
    property bool focused: false
    // Rows and cells select in two washes: the row's accent badge, the cell's
    // quieter surface. The host picks; the rest of the recipe is shared.
    property color selectedFill: CelestinaTheme.badgeAccentFill

    // What the plate — and, if the host binds it, the content — scales to.
    readonly property real recoil: highlight.pressed
                                   ? CelestinaTheme.rowRecoilScale : 1

    radius: CelestinaTheme.radiusSm
    scale: highlight.recoil
    transformOrigin: Item.Center
    color: highlight.dragging
           ? CelestinaTheme.surfaceStrong
           : highlight.pressed
             ? CelestinaTheme.pressedWash
             : highlight.selected
               ? highlight.selectedFill
               : highlight.hovered
                 ? (highlight.family === CelestinaRowHighlight.Content
                    ? CelestinaTheme.contentHover
                    : CelestinaTheme.surfaceHover)
                 : CelestinaTheme.clear
    border.width: highlight.focused ? CelestinaTheme.borderFocus : 0
    border.color: CelestinaTheme.focusRing

    Behavior on color {
        ColorAnimation {
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionFast
        }
    }

    Behavior on scale {
        NumberAnimation {
            // Fast in, slow out: the sink is immediate, the return is felt.
            duration: CelestinaTheme.reducedMotion
                      ? 0 : highlight.pressed ? CelestinaTheme.motionFast
                                              : CelestinaTheme.motionSlow
            easing.type: CelestinaTheme.easeStandard
        }
    }
}
