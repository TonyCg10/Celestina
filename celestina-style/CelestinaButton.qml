import QtQuick
import QtQuick.Controls

// ─── CelestinaButton ──────────────────────────────────────────────────────────
// El botón del suite, en sus tres papeles: normal, principal y destructivo. El
// destructivo no es rojo por decoración — es lo único que distingue "Vaciar
// papelera" de "Cancelar" cuando se lee deprisa. `helpText` cuelga una ayuda
// emergente para los botones sin texto suficiente.
//
// Vive en celestina-style para que las dos apps compartan un solo botón en vez
// de reimplementar cada una el relleno hover/press y el acento primario. Un
// botón que flota sobre contenido desplazable (el selector del portal) es una
// especialización aparte: se queda con su fondo de cristal.
// ──────────────────────────────────────────────────────────────────────────────
Button {
    id: control

    property bool primary: false
    // Variante con contorno de peligro para acciones irreversibles.
    property bool destructive: false
    property string helpText: ""

    hoverEnabled: true
    implicitHeight: 30
    leftPadding: 14
    rightPadding: 14
    font.family: CelestinaTheme.sansFamily
    font.pixelSize: CelestinaTheme.fontRowSecondary
    font.weight: CelestinaTheme.weightMedium

    ToolTip.visible: helpText.length > 0 && hovered
    ToolTip.text: helpText

    contentItem: Text {
        text: control.text
        font: control.font
        color: !control.enabled
               ? CelestinaTheme.textMuted
               : control.primary ? CelestinaTheme.accentInk
               : control.destructive ? CelestinaTheme.dangerFillInk
               : CelestinaTheme.text
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    background: Rectangle {
        radius: CelestinaTheme.radiusSm
        opacity: control.enabled ? 1 : 0.5
        color: {
            if (!control.enabled)
                return CelestinaTheme.controlFill
            if (control.primary)
                return control.down ? Qt.darker(CelestinaTheme.accent, 1.18)
                     : control.hovered ? Qt.darker(CelestinaTheme.accent, 1.08)
                     : CelestinaTheme.accent
            if (control.destructive)
                return control.down ? CelestinaTheme.danger
                     : control.hovered ? CelestinaTheme.dangerBorder
                     : CelestinaTheme.dangerFill
            return control.down ? CelestinaTheme.surfaceStrong
                 : control.hovered ? CelestinaTheme.surfaceHover
                 : CelestinaTheme.controlFill
        }
        border.width: control.activeFocus ? 2 : 0
        border.color: CelestinaTheme.focusRing

        Behavior on color {
            ColorAnimation { duration: CelestinaTheme.motionFast }
        }
    }
}
