import QtQuick
import org.celestina.siderita 1.0

// ─── GlassPill ────────────────────────────────────────────────────────────────
// La pastilla de los controles que flotan sobre el contenido: cristal debajo,
// tinte de estado encima. El orden importa — los tokens de relleno del tema son
// translúcidos, así que el tinte deja ver el desenfoque en vez de taparlo, y una
// pastilla activa sigue siendo reconocible sobre lo que pasa por detrás.
// ──────────────────────────────────────────────────────────────────────────────
Rectangle {
    id: glassPill

    property Item backdrop
    property bool floating: false
    property color fill: CelestinaTheme.controlFill

    radius: CelestinaTheme.radiusSm
    color: CelestinaTheme.clear

    GlassSurface {
        anchors.fill: parent
        backdropSource: glassPill.backdrop
        // Sólo se captura cuando hay contenido detrás que desenfocar: al
        // final de la lista no hay nada bajo el pie y el cristal se apaga.
        captureEnabled: glassPill.floating
        liveCapture: true
        cornerRadius: glassPill.radius
        opacity: glassPill.floating ? 1 : 0
        Behavior on opacity {
            NumberAnimation { duration: CelestinaTheme.motionNormal }
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: glassPill.radius
        color: glassPill.fill
        Behavior on color {
            ColorAnimation { duration: CelestinaTheme.motionFast }
        }
    }
}
