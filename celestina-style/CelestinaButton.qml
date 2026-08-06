import QtQuick
import QtQuick.Controls

// ─── CelestinaButton ──────────────────────────────────────────────────────────
// El botón del suite, con un papel cerrado: tonal, principal, destructivo,
// seleccionado o transparente. Un enum impide combinaciones contradictorias
// como principal + peligro. El destructivo no es rojo por decoración — es lo
// único que distingue "Vaciar
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
        Selected,
        Ghost
    }

    enum Density {
        Compact,
        Regular,
        Prominent
    }

    property int role: CelestinaButton.Tonal
    property int density: CelestinaButton.Compact
    property string helpText: ""

    hoverEnabled: true
    implicitHeight: density === CelestinaButton.Prominent
                    ? CelestinaTheme.controlHeightXl
                    : density === CelestinaButton.Regular
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
               : control.role === CelestinaButton.Destructive
                 ? (control.down ? CelestinaTheme.dangerInk
                                 : CelestinaTheme.dangerFillInk)
               : control.role === CelestinaButton.Selected ? CelestinaTheme.accentLink
               : CelestinaTheme.text
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    background: Rectangle {
        radius: control.density === CelestinaButton.Prominent
                || control.density === CelestinaButton.Regular
                ? CelestinaTheme.radiusButton
                : CelestinaTheme.radiusSm
        opacity: control.enabled ? 1 : CelestinaTheme.disabledOpacity
        color: {
            // A disabled primary keeps its accent wash: `accentDisabledFill`
            // exists for exactly that, and without it the one screen action
            // that is supposed to stand out becomes indistinguishable from
            // every tonal control beside it while it waits to become available.
            //
            // Only the fill. `accentDisabledInk` is the matching label ink, but
            // measured against this fill it reaches 2.6:1-3.5:1 — below the
            // 4.5:1 floor the contract owes normal text in every state — so the
            // label stays on `textMuted`, which measures 5.1:1-7.3:1 over the
            // same fill. The remaining roles have no disabled token of their
            // own, so a disabled destructive still loses its red.
            if (!control.enabled)
                return control.role === CelestinaButton.Primary
                     ? CelestinaTheme.accentDisabledFill
                     : CelestinaTheme.controlFill
            if (control.role === CelestinaButton.Primary)
                return control.down ? CelestinaTheme.accentPressed
                     : control.hovered ? CelestinaTheme.accentHover
                     : CelestinaTheme.accent
            if (control.role === CelestinaButton.Destructive)
                return control.down ? CelestinaTheme.danger
                     : control.hovered ? CelestinaTheme.dangerBorder
                     : CelestinaTheme.dangerFill
            if (control.role === CelestinaButton.Selected)
                return control.down ? CelestinaTheme.surfaceSelected
                     : control.hovered ? CelestinaTheme.accentSoft
                     : CelestinaTheme.badgeAccentFill
            if (control.role === CelestinaButton.Ghost)
                return control.down ? CelestinaTheme.surfaceStrong
                     : control.hovered ? CelestinaTheme.controlFill
                     : CelestinaTheme.clear
            return control.down ? CelestinaTheme.surfaceStrong
                 : control.hovered ? CelestinaTheme.surfaceHover
                 : CelestinaTheme.controlFill
        }
        border.width: 0

        CelestinaFocusRing {
            target: parent
            cornerRadius: parent.radius
            shown: control.visualFocus
        }

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
            }
        }
    }
}
