import QtQuick
import org.celestina.siderita 1.0

// ─── SidebarChevron ───────────────────────────────────────────────────────────
// El galón de las cabeceras plegables del panel lateral. Una sola punta que gira
// en vez de dos glifos que se intercambian: el giro *cuenta* que la zona se
// abre, y el salto entre "▸" y "▾" no cuenta nada.
// ──────────────────────────────────────────────────────────────────────────────
Text {
    // La escala de texto del panel: la trae quien lo usa, porque un tipo en su
    // propio fichero ya no alcanza la ventana de la que salió.
    property real textScale: 1.0

    property bool collapsed: false

    width: 8
    text: "\u25BE"
    color: CelestinaTheme.textMuted
    font.family: CelestinaTheme.sansFamily
    font.pixelSize: Math.round(CelestinaTheme.fontMini * textScale)
    horizontalAlignment: Text.AlignHCenter
    transformOrigin: Item.Center
    rotation: collapsed ? -90 : 0

    Behavior on rotation {
        NumberAnimation {
            duration: CelestinaTheme.motionFast
            easing.type: Easing.OutCubic
        }
    }
}
