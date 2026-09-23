pragma Singleton

import QtQuick

// The one definition of the hidden-entries toggle: which glyph it wears and
// what a screen reader calls it. Two surfaces draw it — the portal picker's
// floating pill and the folder capsule's ghost icon — and a toggle that says
// one thing in one window and another in the next is a defect, not a variant.
QtObject {
    function glyph(checked) {
        return checked ? "eye" : "eye-off"
    }

    function name(checked) {
        return checked ? qsTr("Ocultar elementos ocultos")
                       : qsTr("Mostrar elementos ocultos")
    }
}
