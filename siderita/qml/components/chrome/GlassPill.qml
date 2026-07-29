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
    // Standalone pills float above the file delegates, so they must also be
    // input surfaces.  A visual-only Rectangle lets hover and passive drag
    // handlers below it react through the glass.
    property bool inputShield: true

    radius: CelestinaTheme.radiusPill
    color: CelestinaTheme.clear

    GlassSurface {
        anchors.fill: parent
        backdropSource: glassPill.backdrop
        // Sólo se captura cuando hay contenido detrás que desenfocar: al
        // final de la lista no hay nada bajo el pie y el cristal se apaga.
        captureEnabled: glassPill.floating
        liveCapture: true
        cornerRadius: glassPill.radius
        elevation: glassPill.floating ? 2 : 0
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

    // Keep hover state on the chrome itself instead of also lighting the file
    // row/cell behind it.  Child controls are delivered first and remain fully
    // interactive.
    HoverHandler {
        enabled: glassPill.inputShield
        blocking: true
    }

    // Claim drags that begin on non-interactive space inside a pill.  Without
    // this, the delegate's passive DragHandler can start a file drag through
    // the floating chrome.
    DragHandler {
        enabled: glassPill.inputShield
        target: null
        grabPermissions: PointerHandler.CanTakeOverFromAnything
                         | PointerHandler.ApprovesTakeOverByAnything
    }

    MouseArea {
        anchors.fill: parent
        z: -1
        enabled: glassPill.inputShield
        // History buttons belong to the window. This shield only blocks the
        // ordinary buttons that could otherwise act on a file through chrome.
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        hoverEnabled: true
        preventStealing: true
    }
}
