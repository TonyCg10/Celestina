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
            width: track.width * control.fraction
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
        onClicked: function(mouse) {
            control.focusReason = Qt.MouseFocusReason
            control.forceActiveFocus(Qt.MouseFocusReason)
            control.moveTo(control.from + mouse.x / Math.max(1, control.width) * control.span)
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
}
