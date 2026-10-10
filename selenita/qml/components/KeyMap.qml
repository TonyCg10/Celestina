import QtQuick
import QtQuick.Controls
import org.celestina.selenita 1.0

// The window's keys (design §5.3): 1, 2 and 3 choose the target, Enter takes
// the capture, R toggles the recording and Escape brings the focus back here,
// off whichever control or history row held it. The cards sit inside this
// scope, which is the window's focus item until a control takes it, so a key
// no control wanted bubbles up to this map; a key typed into a text field
// never reaches it. A focused button takes only Space on its own, so Enter
// there presses the button rather than capturing; on a switch, whose Enter
// would mean nothing, and on the map itself Enter captures. `host` is the
// window, or any object with `startCapture()`.
FocusScope {
    id: keys

    required property var host

    // The item holding the window's focus.
    function focused(): Item {
        const window = keys.Window.window
        return window ? window.activeFocusItem : null
    }

    // Whether the item holding the focus takes text, so a digit or a letter
    // there is a character, not a command.
    function editing(): bool {
        const item = keys.focused()
        return item !== null && (item instanceof TextInput || item instanceof TextEdit)
    }

    // Whether a focused item is a button Enter should press: anything with
    // `clicked`, a switch apart. Untyped, as the item may be any control.
    function pressable(item: var): bool {
        return item !== null && item !== keys && !(item instanceof Switch)
               && typeof item.clicked === "function"
    }

    function press(item: var) {
        item.clicked()
    }

    // Enter presses a focused button and captures otherwise.
    function enter() {
        const item = keys.focused()
        if (keys.pressable(item))
            keys.press(item)
        else
            keys.host.startCapture()
    }

    focus: true

    Keys.onPressed: event => {
        if (keys.editing() || (event.modifiers & ~Qt.KeypadModifier) !== 0)
            return
        switch (event.key) {
        case Qt.Key_1:
            SelenitaController.target = "screen"
            break
        case Qt.Key_2:
            SelenitaController.target = "window"
            break
        case Qt.Key_3:
            SelenitaController.target = "region"
            break
        case Qt.Key_Return:
        case Qt.Key_Enter:
            keys.enter()
            break
        case Qt.Key_R:
            SelenitaController.toggleRecording()
            break
        case Qt.Key_Escape: {
            // The scope keeps the focus; the control inside it lets go.
            const item = keys.focused()
            if (item && item !== keys)
                item.focus = false
            keys.forceActiveFocus(Qt.OtherFocusReason)
            break
        }
        default:
            return
        }
        event.accepted = true
    }
}
