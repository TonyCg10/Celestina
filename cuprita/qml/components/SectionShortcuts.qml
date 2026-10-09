import QtQuick
import QtQuick.Controls
import org.celestina.cuprita 1.0

// Ctrl+1 to Ctrl+4, one per section. An Item, so the shortcuts have
// the window as their context wherever it is placed.
Item {
    id: shortcuts

    signal activated(int index)

    Shortcut {
        sequence: "Ctrl+1"
        onActivated: shortcuts.activated(0)
    }

    Shortcut {
        sequence: "Ctrl+2"
        onActivated: shortcuts.activated(1)
    }

    Shortcut {
        sequence: "Ctrl+3"
        onActivated: shortcuts.activated(2)
    }

    Shortcut {
        sequence: "Ctrl+4"
        onActivated: shortcuts.activated(3)
    }
}
