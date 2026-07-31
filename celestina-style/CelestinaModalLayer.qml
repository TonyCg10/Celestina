import QtQuick
import QtQuick.Window

// Shared L3 interaction layer: scrim, fade, focus and dismissal semantics.
// Dialog-specific size, content and controller actions stay with the app.
FocusScope {
    id: layer

    property bool shown: false
    property bool dismissOnOutsideClick: true
    property bool dismissOnEscape: true
    property Item previousFocusItem: null
    property Item pendingRestoreFocusItem: null
    property Item lastOwnedFocusItem: null
    property color color: CelestinaTheme.scrim
    signal dismissRequested

    visible: shown || opacity > 0.01
    opacity: shown ? 1 : 0

    function ownsItem(item) {
        let current = item
        while (current) {
            if (current === layer)
                return true
            current = current.parent
        }
        return false
    }

    function containsItem(container, item) {
        let current = item
        while (current) {
            if (current === container)
                return true
            current = current.parent
        }
        return false
    }

    function appendFocusable(item, result) {
        if (!item || (item !== layer && (!item.visible || !item.enabled)))
            return
        if (item !== layer && item.activeFocusOnTab) {
            result.push(item)
            return
        }
        for (let index = 0; index < item.children.length; ++index)
            appendFocusable(item.children[index], result)
    }

    function focusInside(forward) {
        const window = layer.Window.window
        const current = window ? window.activeFocusItem : null
        const focusable = []
        appendFocusable(layer, focusable)
        if (focusable.length === 0) {
            layer.forceActiveFocus(Qt.PopupFocusReason)
            return
        }

        let currentIndex = -1
        for (let index = 0; index < focusable.length; ++index) {
            if (containsItem(focusable[index], current)) {
                currentIndex = index
                break
            }
        }
        const nextIndex = currentIndex < 0
                          ? (forward ? 0 : focusable.length - 1)
                          : (currentIndex + (forward ? 1 : -1)
                             + focusable.length) % focusable.length
        focusable[nextIndex].forceActiveFocus(
                    forward ? Qt.TabFocusReason : Qt.BacktabFocusReason)
    }

    function keepFocusInside() {
        const window = layer.Window.window
        const current = window ? window.activeFocusItem : null
        if (ownsItem(current)) {
            lastOwnedFocusItem = current
            return
        }

        const focusable = []
        appendFocusable(layer, focusable)
        if (focusable.length === 0) {
            layer.forceActiveFocus(Qt.PopupFocusReason)
            return
        }

        let previousIndex = -1
        for (let index = 0; index < focusable.length; ++index) {
            if (containsItem(focusable[index], lastOwnedFocusItem)) {
                previousIndex = index
                break
            }
        }
        const backward = previousIndex === 0
        const target = backward ? focusable[focusable.length - 1]
                                : focusable[0]
        Qt.callLater(function() {
            const activeWindow = layer.Window.window
            if (layer.shown && (!activeWindow
                                || !layer.ownsItem(activeWindow.activeFocusItem)))
                target.forceActiveFocus(backward ? Qt.BacktabFocusReason
                                                 : Qt.TabFocusReason)
        })
    }

    function restorePendingFocus() {
        const target = pendingRestoreFocusItem
        pendingRestoreFocusItem = null
        if (target)
            Qt.callLater(function() {
                if (!layer.shown && !layer.visible
                        && target.visible && target.enabled)
                    target.forceActiveFocus(Qt.PopupFocusReason)
            })
    }

    onShownChanged: {
        if (shown) {
            pendingRestoreFocusItem = null
            lastOwnedFocusItem = null
            const window = layer.Window.window
            previousFocusItem = window ? window.activeFocusItem : null
            Qt.callLater(function() {
                const activeWindow = layer.Window.window
                if (layer.shown && (!activeWindow
                                    || !layer.ownsItem(activeWindow.activeFocusItem)))
                    layer.focusInside(true)
            })
            return
        }

        pendingRestoreFocusItem = previousFocusItem
        previousFocusItem = null
        lastOwnedFocusItem = null
        if (!visible)
            restorePendingFocus()
    }
    onVisibleChanged: if (!visible) restorePendingFocus()

    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.reducedMotion
                      ? 0 : CelestinaTheme.motionFast
            easing.type: CelestinaTheme.easeStandard
        }
    }

    Rectangle {
        anchors.fill: parent
        color: layer.color
    }

    Connections {
        target: layer.Window.window
        function onActiveFocusItemChanged() {
            if (layer.shown)
                layer.keepFocusInside()
        }
    }

    // ── Input shield ─────────────────────────────────────────────────────
    // A scrim that only catches left clicks is not a modal layer. Two things
    // leak through one: the other mouse buttons and hover, and — the one that
    // actually bites — the *pointer handlers* of the surface below. A
    // `DragHandler` down there takes a passive grab on the press and keeps
    // reacting to the drag, so sweeping over an empty part of a dialog card
    // dragged the file the card was covering.
    //
    // Hover and that drag claim are the shared `CelestinaInputShield`; the
    // click side stays here because this layer does more than swallow — an
    // outside click is its dismissal, and the wheel must not scroll a surface
    // the dialog is blocking. Both sit at `z: -1`, below the dialog's own
    // content: everything inside the layer is delivered first and stays fully
    // interactive, and only what the dialog did not claim is absorbed.
    Item {
        anchors.fill: parent
        z: -1
        // Stay armed until the exit fade has left the scene, so the surface
        // below cannot be poked through a dialog that is still painted.
        enabled: layer.visible

        CelestinaInputShield {
            swallowClicks: false
        }

        MouseArea {
            anchors.fill: parent
            // All three buttons: a right click landing on a file behind a
            // dialog would open that file's menu, which is exactly the kind of
            // surprise a modal exists to prevent.
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
            hoverEnabled: true
            preventStealing: true
            // Closing is no longer an actionable outside click: it must not
            // emit a second dismissal while `shown` is already false. Only the
            // left button dismisses — the others are swallowed, not acted on.
            onClicked: function(mouse) {
                if (mouse.button === Qt.LeftButton && layer.shown
                        && layer.dismissOnOutsideClick)
                    layer.dismissRequested()
            }
            onWheel: function(wheel) { wheel.accepted = true }
        }
    }

    Keys.priority: Keys.BeforeItem
    Keys.onPressed: function(event) {
        if (!layer.shown)
            return
        if (layer.dismissOnEscape && event.key === Qt.Key_Escape) {
            layer.dismissRequested()
            event.accepted = true
        }
    }
}
