import QtQuick
import org.celestina.fluorita 1.0

// El transporte de Fluorita sobre el control compartido.
//
// La anatomía —pista, relleno, marca de lo pendiente, anillo de foco, teclado—
// vive en `CelestinaSlider` desde que un segundo consumidor (el reproductor
// incrustado de Siderita) demostró la misma semántica. Aquí quedan sólo las
// palabras del medio: qué es este valor y qué significan sus flechas.
CelestinaSlider {
    id: bar

    // The wheel seeks: reaching for it while watching is not a request to
    // first find and focus the bar.
    wheelEnabled: true

    required property real position
    required property real duration
    // Una búsqueda pedida y no confirmada. El relleno se queda donde el motor
    // dijo; esto se dibuja aparte para que la interfaz nunca afirme que la
    // cabeza ya se movió.
    property real pendingPosition: -1

    signal seekRequested(real seconds)

    // The shared slider centres its track in whatever height it is given: a
    // control the height of a button is hit by the pointer, the bare 16 px
    // track is not.
    implicitHeight: CelestinaTheme.controlHeightXs

    value: bar.position
    to: bar.duration
    pendingValue: bar.pendingPosition
    // Un paso que se nota sin perder el sitio.
    step: 5

    Accessible.name: qsTr("Posición")
    Accessible.description: qsTr("Flechas para avanzar o retroceder cinco segundos")

    onMoved: function(target) { bar.seekRequested(target) }
}
