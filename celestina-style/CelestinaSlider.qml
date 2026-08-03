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

    implicitHeight: CelestinaTheme.spaceLg
    activeFocusOnTab: control.enabled

    Accessible.role: Accessible.Slider
    Accessible.focusable: control.enabled

    Rectangle {
        id: track

        anchors.verticalCenter: parent.verticalCenter
        width: parent.width
        height: CelestinaTheme.spaceXs
        radius: CelestinaTheme.radiusPill
        color: CelestinaTheme.divider

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
    }

    CelestinaFocusRing {
        target: track
        cornerRadius: track.radius
        shown: control.activeFocus && control.focusReason !== Qt.MouseFocusReason
    }

    MouseArea {
        anchors.fill: parent
        enabled: control.enabled
        // A click alone only jumped once; nothing followed the cursor while
        // the button stayed down. `onPositionChanged` on a `MouseArea` with
        // `hoverEnabled` false (the default, unset here) only fires while a
        // button is held, which is exactly a drag — so the same math runs on
        // press and on every move, both driving the local drag position and
        // asking the consumer to move there for real.
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
        control.dragFraction = Math.max(0, Math.min(1, x / Math.max(1, control.width)))
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
