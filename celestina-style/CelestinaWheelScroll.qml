import QtQuick

// Wheel scrolling for any Flickable, ListView or GridView. Each notch adds
// to a destination the view approaches by a fixed fraction of the remaining
// distance every frame: it moves on the next frame, covers 90 % in about
// 60 ms and has no tail, and a burst of notches adds up exactly. Qt's own
// wheel handling turns a notch into a decelerating flick (a ~300 ms tail
// that loses most of a fast burst), which is what the author felt as a
// brake. Touchpad pixel deltas go straight through. At a bound the event is
// left for an outer scroller, and so is a touchpad swipe that is mostly
// horizontal, which belongs to whatever scrolls sideways.
//
// It is a MouseArea, not a WheelHandler: a WheelHandler decides whether the
// event goes on before its `wheel` signal runs (`blocking`), so it cannot
// pass on a notch that would push past a bound, and an outer scroller never
// saw it. The area lives in the view's contentItem, below the delegates, and
// covers exactly the visible part: delegates and their handlers see the wheel
// first, then this area, then the Flickable's own handling (a child of the
// Flickable itself would come after it). It takes no buttons and no hover, so
// presses and drags still reach the view.
MouseArea {
    id: root

    required property Flickable view
    property real step: CelestinaTheme.compWheelStep
    // Modifier combinations this scroller takes; anything else (Ctrl+wheel
    // zoom, for instance) is left for the handlers behind it.
    property int acceptedModifiers: Qt.NoModifier
    property real targetY: 0
    property real written: NaN
    signal stepped(real delta, bool notch)

    parent: root.view.contentItem
    x: root.view.contentX
    y: root.view.contentY
    width: root.view.width
    height: root.view.height
    z: -1
    acceptedButtons: Qt.NoButton
    hoverEnabled: false

    function minY() { return root.view.originY - root.view.topMargin }
    function maxY() {
        return Math.max(root.minY(), root.view.originY + root.view.contentHeight
                        + root.view.bottomMargin - root.view.height)
    }
    function clampY(y) { return Math.max(root.minY(), Math.min(root.maxY(), y)) }
    function put(y) { root.written = y; root.view.contentY = y }
    function retarget() {
        root.targetY = root.clampY(root.targetY)
        if (!glide.running)
            root.put(root.clampY(root.view.contentY))
    }

    onWheel: function(event) {
        if (!root.view || event.modifiers !== root.acceptedModifiers) {
            event.accepted = false
            return
        }
        if (Math.abs(event.pixelDelta.x) > Math.abs(event.pixelDelta.y)) {
            event.accepted = false
            return
        }
        const pixel = Math.abs(event.pixelDelta.y) >= 0.01
        const delta = pixel ? event.pixelDelta.y : event.angleDelta.y / 120 * root.step
        if (Math.abs(delta) < 0.01) { event.accepted = false; return }
        root.stepped(delta, !pixel)
        if (!glide.running)
            root.targetY = root.view.contentY
        const next = root.clampY(root.targetY - delta)
        if (Math.abs(next - root.targetY) < 0.01
                && Math.abs(next - root.view.contentY) < 0.5) {
            event.accepted = false
            return
        }
        root.view.cancelFlick()
        root.targetY = next
        if (pixel || CelestinaTheme.reducedMotion) {
            glide.stop()
            root.put(root.targetY)
        } else {
            glide.start()
        }
        event.accepted = true
    }

    property FrameAnimation glide: FrameAnimation {
        onTriggered: {
            if (!root.view) {
                stop()
                return
            }
            // Something else moved the view (scrollbar, keys): it wins.
            if (currentFrame > 1 && !isNaN(root.written)
                    && Math.abs(root.view.contentY - root.written) > 0.5) {
                stop()
                return
            }
            root.targetY = root.clampY(root.targetY)
            const remaining = root.targetY - root.view.contentY
            if (Math.abs(remaining) < 0.5) {
                root.put(root.targetY)
                stop()
                return
            }
            root.put(root.view.contentY + remaining
                     * (1 - Math.exp(-frameTime * 1000 / CelestinaTheme.wheelFollowMs)))
        }
        onRunningChanged: if (!running) root.written = NaN
    }
}
