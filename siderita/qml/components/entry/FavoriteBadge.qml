import QtQuick
import org.celestina.siderita 1.0

// ─── FavoriteBadge ────────────────────────────────────────────────────────────
// La estrella que marca un favorito, montada sobre la esquina de su icono. Usa
// la estrella del propio Siderita y no la del tema de iconos: esto es chrome de
// la aplicación, y la regla de "no tiñas los iconos" habla del icono de una
// entrada, no de una insignia que la aplicación dibuja encima.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    // La escala de iconos del contenido, traída por quien lo usa.
    property real iconScale: 1.0

    required property bool starred
    // Sized by the caller against its tile — the list's glyph is half the
    // grid's — and it rides the content-icon slider like everything else.
    property int diameter: Math.round(19 * iconScale)

    visible: starred
    width: diameter
    height: diameter

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        color: CelestinaTheme.favoriteBadgeFill
    }

    // The bundled star, not the icon theme's: this is Siderita's own chrome
    // (like the play badge), so it should look the same under any theme —
    // the "don't tint" rule is about an entry's own icon, not about a badge.
    CelestinaIcon {
        anchors.centerIn: parent
        width: Math.round(parent.width * 0.72)
        height: width
        fallbackName: "star"
        tone: CelestinaIcon.Favorite
    }
}
