import QtQuick
import QtQuick.Controls

// ─── CelestinaButton ──────────────────────────────────────────────────────────
// El botón del suite, con un papel cerrado: tonal, principal, destructivo o de
// texto. Un enum impide combinaciones contradictorias como principal + peligro.
// El
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

    enum Role {
        Tonal,
        Primary,
        Destructive,
        TextOnly
    }

    enum Density {
        Compact,
        Regular
    }

    property int role: CelestinaButton.Tonal
    property int density: CelestinaButton.Compact
    property string helpText: ""

    hoverEnabled: true
    implicitHeight: density === CelestinaButton.Regular
                    ? CelestinaTheme.controlHeight
                    : CelestinaTheme.controlHeightXs
    leftPadding: CelestinaTheme.compButtonPaddingHorizontal
    rightPadding: CelestinaTheme.compButtonPaddingHorizontal
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
               : control.role === CelestinaButton.Primary ? CelestinaTheme.accentInk
               : control.role === CelestinaButton.Destructive ? CelestinaTheme.dangerFillInk
               : CelestinaTheme.text
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    background: Rectangle {
        radius: control.density === CelestinaButton.Regular
                ? CelestinaTheme.radiusButton
                : CelestinaTheme.radiusSm
        opacity: control.enabled ? 1 : CelestinaTheme.disabledOpacity
        color: {
            if (!control.enabled)
                return CelestinaTheme.controlFill
            if (control.role === CelestinaButton.Primary)
                return control.down ? CelestinaTheme.accentPressed
                     : control.hovered ? CelestinaTheme.accentHover
                     : CelestinaTheme.accent
            if (control.role === CelestinaButton.Destructive)
                return control.down ? CelestinaTheme.danger
                     : control.hovered ? CelestinaTheme.dangerBorder
                     : CelestinaTheme.dangerFill
            if (control.role === CelestinaButton.TextOnly)
                return control.down ? CelestinaTheme.surfaceStrong
                     : control.hovered ? CelestinaTheme.controlFill
                     : CelestinaTheme.clear
            return control.down ? CelestinaTheme.surfaceStrong
                 : control.hovered ? CelestinaTheme.surfaceHover
                 : CelestinaTheme.controlFill
        }
        border.width: control.activeFocus ? CelestinaTheme.borderFocus : 0
        border.color: CelestinaTheme.focusRing

        Behavior on color {
            ColorAnimation { duration: CelestinaTheme.motionFast }
        }
    }
}
