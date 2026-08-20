import QtQuick

// Which of the three heading states a folder is in, and the four transitions
// between them.
//
// It lives apart from the view because it is a state machine, not layout: the
// rules — an expanded heading always yields, a compact one only to a deliberate
// gesture, arriving at the top restores it, a further push there expands it —
// are the kind of thing that has to be read in one place to stay coherent.
QtObject {
    id: root

    // Guards from the view: a heading is never revealed while the folder is
    // loading, has failed, or is not the active tab.
    required property bool canReveal

    property bool expanded: false
    property bool retired: false

    // Any downward scroll. The metadata block gets out of the way at once.
    function collapse() {
        expanded = false
    }

    // Enough downward travel to mean it: the default title goes too, and the
    // listing takes its band. Never straight from expanded — one gesture makes
    // one change.
    function retire() {
        if (!expanded)
            retired = true
    }

    // Reaching the top of the listing. Brings back the default title, never the
    // expanded one.
    function restore() {
        retired = false
    }

    // A further push once at the top.
    function reveal() {
        if (!root.canReveal)
            return
        retired = false
        expanded = true
    }
}
