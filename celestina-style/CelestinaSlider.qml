import QtQuick

// ─── CelestinaSlider ─────────────────────────────────────────────────────────
// El control de valor continuo del sistema: pista, relleno y —cuando el
// consumidor lo pide— una marca separada para un valor *solicitado pero no
// confirmado*.
//
// Esa marca es la razón de que no sea un `Slider` de Qt Controls, además del
// trinquete que mantiene fuera los controles Qt reconstruidos: un reproductor
// tiene que poder enseñar dónde dice el motor que está la cabeza y dónde pidió
// ir un clic, sin que lo segundo finja ser lo primero. Un consumidor que no
// necesite eso simplemente no toca `pendingValue`.
//
// El texto accesible lo pone el consumidor: el nombre de este valor y el paso
// que significan sus flechas son suyos, no del sistema visual.
// ─────────────────────────────────────────────────────────────────────────────
Item {
    id: control

    required property real value
    // `to` a 0 deja el control deshabilitado de hecho: no hay recorrido.
    required property real to
    property real from: 0
    // Lo que mueven las flechas. En unidades del valor, no en píxeles.
    property real step: 1
    // Un valor pedido y aún sin confirmar, o negativo para no dibujarlo.
    property real pendingValue: -1
    // A wheel notch over the control moves one `step`. Off by default: a host
    // whose row already answers the wheel with its own rounding would otherwise
    // see the slider answer first with a different one. A media bar that wants
    // the wheel turns it on.
    property bool wheelEnabled: false

    signal moved(real value)

    readonly property real span: Math.max(0, control.to - control.from)
    readonly property real fraction: control.span > 0
        ? Math.max(0, Math.min(1, (control.value - control.from) / control.span))
        : 0
    readonly property real pendingFraction: control.span > 0 && control.pendingValue >= 0
        ? Math.max(0, Math.min(1, (control.pendingValue - control.from) / control.span))
        : -1

    // While the mouse is down, the thumb follows the cursor on the spot —
    // `value` only moves once whatever owns it (a confirmed engine report,
    // for a seek bar) comes back, which would otherwise make a drag lag
    // behind the cursor or not visibly track it at all. This is purely a
    // local, ephemeral display: it never substitutes for `value`, and it
    // snaps back to whatever `value` says the moment the button is let go.
    property bool dragging: false
    property real dragFraction: 0
    readonly property real displayFraction: control.dragging ? control.dragFraction : control.fraction

    // A fast drag moves the cursor across many pixels before the next paint,
    // and asking the consumer to act on every one of them queued far more
    // seeks than a video's decoder could keep up with — so after letting go,
    // the picture kept catching up through that backlog on its own, looking
    // exactly like inertia. The visual thumb above still tracks every move;
    // only how often `moved` actually fires is capped, well under what a
    // person can perceive as a delay.
    property real dragTarget: 0
    property bool dragTargetPending: false

    Timer {
        interval: 80
        repeat: true
        running: control.dragging
        onTriggered: control.flushDrag()
    }

    // Qt dice por qué llegó el foco; un clic no debe levantar el anillo.
    property int focusReason: Qt.OtherFocusReason

    // Whether the pointer is over the control. The hover state of the track
    // and thumb below reads it; a consumer that needs its own treatment can
    // too.
    readonly property alias hovered: pointer.containsMouse
    readonly property int handleSize: CelestinaTheme.compSliderHandleSize

    // As tall as the other compact controls, so the strip that answers a press
    // is the one the other controls answer in; the track stays a hairline
    // centred inside it.
    implicitHeight: CelestinaTheme.controlHeightXs
    activeFocusOnTab: control.enabled

    Accessible.role: Accessible.Slider
    Accessible.focusable: control.enabled

    // The track is inset by half a thumb on each side so the thumb at either
    // end still sits inside the control's box — a slider laid out beside a
    // button must not paint over it. `dragTo` maps the pointer against this
    // same inset, so the thumb lands under the pointer end to end.
    Rectangle {
        id: track

        anchors.verticalCenter: parent.verticalCenter
        x: control.handleSize / 2
        width: Math.max(0, parent.width - control.handleSize)
        height: CelestinaTheme.spaceXs
        radius: CelestinaTheme.radiusPill
        color: control.enabled && (control.hovered || control.dragging)
               ? CelestinaTheme.inputBorder : CelestinaTheme.divider

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
            }
        }

        Rectangle {
            width: track.width * control.displayFraction
            height: track.height
            radius: track.radius
            color: control.enabled ? CelestinaTheme.accent : CelestinaTheme.textFaint
        }

        // Dónde se pidió ir, mientras la petición sigue pendiente.
        Rectangle {
            visible: control.pendingFraction >= 0
            x: track.width * Math.max(0, control.pendingFraction) - width / 2
            anchors.verticalCenter: track.verticalCenter
            width: CelestinaTheme.spaceSm
            height: CelestinaTheme.spaceSm
            radius: CelestinaTheme.radiusPill
            color: CelestinaTheme.textMuted
        }

        // The thumb: where the value is, and the thing a pointer reaches for.
        // It rides `displayFraction` so it follows the cursor during a drag
        // exactly as the fill does. Hover lifts its colour one step, a drag
        // lifts it further and grows it slightly, the way a grabbed handle
        // does — the same accent derivations `CelestinaButton` uses for its
        // hover and pressed fills, never a local mix.
        Rectangle {
            id: thumb
            objectName: "sliderThumb"

            x: track.width * control.displayFraction - width / 2
            anchors.verticalCenter: track.verticalCenter
            width: control.handleSize
            height: control.handleSize
            radius: CelestinaTheme.radiusPill
            scale: control.dragging ? 1.15 : 1
            color: !control.enabled ? CelestinaTheme.textFaint
                 : control.dragging ? CelestinaTheme.accentLink
                 : control.hovered ? CelestinaTheme.accentHover
                 : CelestinaTheme.accent

            Behavior on color {
                ColorAnimation {
                    duration: CelestinaTheme.reducedMotion
                              ? 0 : CelestinaTheme.motionFast
                }
            }
            Behavior on scale {
                enabled: !CelestinaTheme.reducedMotion
                NumberAnimation { duration: CelestinaTheme.motionFast }
            }
        }
    }

    CelestinaFocusRing {
        target: track
        cornerRadius: track.radius
        shown: control.activeFocus && control.focusReason !== Qt.MouseFocusReason
    }

    // A wheel notch is one keyboard step, in the same units. Discrete notches
    // are 120 units; a touchpad's finer deltas accumulate until they add up to
    // one, so a flick does not spray a burst of requests.
    WheelHandler {
        property real notches: 0

        enabled: control.enabled && control.wheelEnabled
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onWheel: function(event) {
            notches += event.angleDelta.y / 120
            while (notches >= 1) {
                notches -= 1
                control.moveTo(control.value + control.step)
            }
            while (notches <= -1) {
                notches += 1
                control.moveTo(control.value - control.step)
            }
        }
    }

    MouseArea {
        id: pointer

        anchors.fill: parent
        enabled: control.enabled
        // Hover is wanted here for the track and thumb treatment, which means
        // `onPositionChanged` now fires for every move, not only while a
        // button is held — so the drag math runs only while `dragging`, which
        // is exactly the span between press and release.
        hoverEnabled: true
        onPressed: function(mouse) {
            control.focusReason = Qt.MouseFocusReason
            control.forceActiveFocus(Qt.MouseFocusReason)
            control.dragging = true
            control.dragTo(mouse.x)
            // The initial press jumps immediately — only the *following*
            // stream of moves is throttled, so a plain click never waits.
            control.flushDrag()
        }
        onPositionChanged: function(mouse) {
            if (control.dragging)
                control.dragTo(mouse.x)
        }
        onReleased: {
            control.dragging = false
            // Whatever pixel the button came up on is the one that must
            // land, even if it fell between two throttled ticks.
            control.flushDrag()
        }
        onCanceled: {
            control.dragging = false
            control.flushDrag()
        }
    }

    onActiveFocusChanged: if (!control.activeFocus) control.focusReason = Qt.OtherFocusReason

    Keys.onLeftPressed: control.moveTo(control.value - control.step)
    Keys.onRightPressed: control.moveTo(control.value + control.step)
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Home) {
            control.moveTo(control.from)
            event.accepted = true
        } else if (event.key === Qt.Key_End && control.span > 0) {
            control.moveTo(control.to)
            event.accepted = true
        }
    }

    function moveTo(target) {
        if (!control.enabled || control.span <= 0)
            return
        control.moved(Math.max(control.from, Math.min(control.to, target)))
    }

    function dragTo(x) {
        if (!control.enabled || control.span <= 0)
            return
        control.dragFraction = Math.max(0, Math.min(1, (x - track.x) / Math.max(1, track.width)))
        control.dragTarget = control.from + control.dragFraction * control.span
        control.dragTargetPending = true
    }

    function flushDrag() {
        if (!control.dragTargetPending)
            return
        control.dragTargetPending = false
        control.moveTo(control.dragTarget)
    }
}
