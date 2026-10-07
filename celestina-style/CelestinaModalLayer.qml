import QtQuick
import QtQuick.Window

// Shared L3 interaction layer: scrim, fade, focus and dismissal semantics.
// Dialog-specific size, content and controller actions stay with the app.
//
// The consumer's children land in `contentHost`, not in the layer itself, so
// the layer can disable them the moment `shown` drops while the scrim and the
// shield below stay armed through the exit fade. Two different things: a
// dialog that is fading out must neither be pokeable itself — a double click
// on its primary button was sending the request twice — nor let the surface
// under it be poked through the still-painted scrim.
FocusScope {
    id: layer

    default property alias content: contentHost.data

    property bool shown: false
    property bool dismissOnOutsideClick: true
    property bool dismissOnEscape: true
    property Item previousFocusItem: null
    property Item pendingRestoreFocusItem: null
    property Item lastOwnedFocusItem: null
    property color color: CelestinaTheme.scrim
    signal dismissRequested
    // Whether the stage is being rendered live or as the frozen last frame.
    readonly property alias snapshotLive: snapshot.live

    visible: shown || opacity > 0.01
    opacity: shown ? 1 : 0

    // Where the layer parks the focus when nobody asked for a control: on
    // show, and whenever the focused item vanishes (a list rebuilding its
    // rows destroys the delegate that held it). A FocusScope given the focus
    // hands it on to whatever descendant last held it — a list's cursor row,
    // say — and that row lights its ring on an open nobody tabbed into. The
    // sink is a plain item, so the focus stays on it until a Tab moves on.
    Item {
        id: focusSink
        objectName: "celestina-modal-focus-sink"
        width: 0
        height: 0
    }

    function parkFocus() {
        focusSink.forceActiveFocus(Qt.PopupFocusReason)
    }

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

    // `reason` defaults to the keyboard reasons Tab travel deserves; the
    // opening placement passes PopupFocusReason so no ring lights on show.
    function focusInside(forward, reason) {
        const window = layer.Window.window
        const current = window ? window.activeFocusItem : null
        const focusable = []
        appendFocusable(layer, focusable)
        if (focusable.length === 0) {
            layer.parkFocus()
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
                    reason !== undefined ? reason
                    : forward ? Qt.TabFocusReason : Qt.BacktabFocusReason)
    }

    function keepFocusInside() {
        const window = layer.Window.window
        const current = window ? window.activeFocusItem : null
        if (ownsItem(current)) {
            lastOwnedFocusItem = current
            return
        }
        // No item at all holds the focus: the one that did has gone, not
        // travelled. That is a rebuild, not a Tab, so nothing may light.
        if (!current) {
            layer.parkFocus()
            return
        }

        const focusable = []
        appendFocusable(layer, focusable)
        if (focusable.length === 0) {
            layer.parkFocus()
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
        // The first placement on show is not a keystroke: it must not light
        // the focus ring. Only a wrap after real Tab travel keeps a keyboard
        // reason, so the ring follows the key and never the opening.
        const reason = lastOwnedFocusItem === null
                       ? Qt.PopupFocusReason
                       : (backward ? Qt.BacktabFocusReason : Qt.TabFocusReason)
        Qt.callLater(function() {
            const activeWindow = layer.Window.window
            if (layer.shown && (!activeWindow
                                || !layer.ownsItem(activeWindow.activeFocusItem)))
                target.forceActiveFocus(reason)
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
            // The focus is parked on show, not placed on a control: a ring
            // that lights on a field or a button nobody pressed, and then
            // hops to the next one, is what the author saw on every open.
            // Tab travel starts from the sink and reaches the first control;
            // Escape works from there.
            Qt.callLater(function() {
                const activeWindow = layer.Window.window
                if (layer.shown && (!activeWindow
                                    || !layer.ownsItem(activeWindow.activeFocusItem)))
                    layer.parkFocus()
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

    // `motionNormal`, not `motionFast`: at 100 ms the fade read as a cut.
    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.reducedMotion
                      ? 0 : CelestinaTheme.motionNormal
            easing.type: CelestinaTheme.easeStandard
        }
    }

    Connections {
        target: layer.Window.window
        function onActiveFocusItemChanged() {
            if (layer.shown)
                layer.keepFocusInside()
        }
    }

    // Everything the layer paints — scrim, dialog, shield — lives on this
    // stage, and the stage is only ever seen through `snapshot` below. While
    // the dialog is up the snapshot is live, so it is the stage frame by
    // frame; the moment `shown` drops it freezes, and what fades out is the
    // last complete picture of the dialog. That is the only way the fade is
    // one image: a dialog closing also tears down its content (a controller
    // clears its fields, a section folds, the glass stops capturing), and
    // fading the live items let the author see the card shrink, the scrim
    // cut out and the text outlast its tint. Per-item opacity never composes
    // first; a frozen texture already has.
    Item {
        id: stage
        anchors.fill: parent

        Rectangle {
            anchors.fill: parent
            color: layer.color
        }

        // Where the dialog's own content lives. Its `parent` chain still
        // passes through the layer, so `ownsItem` and the focus walk see
        // nothing new; it only adds the one switch a fading dialog needs. A
        // disabled item is skipped for pointer delivery, so a click during
        // the fade falls through to the scrim's MouseArea below, which
        // already declines to act once `shown` is false.
        Item {
            id: contentHost

            anchors.fill: parent
            enabled: layer.shown
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
    }

    ShaderEffectSource {
        id: snapshot
        anchors.fill: parent
        sourceItem: stage
        hideSource: true
        live: layer.shown
        smooth: true
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
