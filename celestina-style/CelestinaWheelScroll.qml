import QtQuick

// Wheel scrolling for any Flickable, ListView or GridView. Each notch adds
// to a destination the view approaches by a fixed fraction of the remaining
// distance every frame: it moves on the next frame, covers 90 % in about
// 60 ms and has no tail, and a burst of notches adds up exactly. Qt's own
// wheel handling turns a notch into a decelerating flick (a ~300 ms tail
// that loses most of a fast burst), which is what the author felt as a
// brake. Touchpad pixel deltas go straight through. At a bound the event is
// left for an outer scroller, and so is a gesture that is mostly horizontal,
// which belongs to whatever scrolls sideways, but only when the view can
// scroll sideways itself (contentWidth > width); on any other view that
// gesture counts as vertical (its y part is applied and `stepped` emitted) so
// rows never move without the signal.
//
// Notch or touchpad is decided by the scroll phase, not by the pixel delta.
// Under Qt 6.12 on the author's session (2026-10-11, 779 traced events in
// Siderita) a mouse-wheel notch arrives with angleDelta.y = ±120 AND
// pixelDelta.y = ±15, phase Qt.NoScrollPhase, and its device reports type
// TouchPad and the name "touchpad", so neither the pixel delta nor the device
// tells a wheel from a touchpad. Taking every event with a pixel delta as a
// touchpad moved each notch 15 px straight instead of a step and a glide,
// the brake the author still felt in every app. A touchpad on Wayland sends
// a phased stream (ScrollBegin, ScrollUpdate, ScrollEnd, ScrollMomentum):
// only a phased event, or one with pixels and no angle at all, is applied in
// pixels. Anything else is a notch: angleDelta / 120 * step, so the fraction
// a high-resolution wheel reports moves the same fraction of a step, and the
// sideways test compares the angles too. `take` holds the whole decision so
// the tests can feed it the fields a real event carries.
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
        // Idle: write only when the clamp really moves the view, because
        // Qt's setContentY ends a running flick even for an unchanged value.
        if (!glide.running) {
            const y = root.clampY(root.view.contentY)
            if (Math.abs(y - root.view.contentY) > 0.01)
                root.put(y)
        }
    }

    // Takes one wheel event, given its fields; returns whether it was used.
    // A false return leaves the event for whatever is behind this area.
    function take(angleX, angleY, pixelX, pixelY, phase, modifiers) {
        if (!root.view || modifiers !== root.acceptedModifiers)
            return false
        const pixel = phase !== Qt.NoScrollPhase
                || (Math.abs(angleX) < 0.01 && Math.abs(angleY) < 0.01)
        const dx = pixel ? pixelX : angleX
        const dy = pixel ? pixelY : angleY
        if (Math.abs(dx) > Math.abs(dy) && root.view.contentWidth > root.view.width)
            return false
        const delta = pixel ? pixelY : angleY / 120 * root.step
        if (Math.abs(delta) < 0.01)
            return false
        root.stepped(delta, !pixel)
        if (!glide.running)
            root.targetY = root.view.contentY
        const next = root.clampY(root.targetY - delta)
        if (Math.abs(next - root.targetY) < 0.01
                && Math.abs(next - root.view.contentY) < 0.5)
            return false
        root.view.cancelFlick()
        root.targetY = next
        if (pixel || CelestinaTheme.reducedMotion) {
            glide.stop()
            root.put(root.targetY)
        } else {
            glide.start()
        }
        return true
    }

    onWheel: function(event) {
        event.accepted = root.take(event.angleDelta.x, event.angleDelta.y,
                                   event.pixelDelta.x, event.pixelDelta.y,
                                   event.phase, event.modifiers)
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
