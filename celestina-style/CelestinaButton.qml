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
//
// Three states share one vocabulary here so no consumer has to invent them:
// hover tints the fill, a press darkens it *and* sinks the whole button by
// `pressRecoilScale` — the recoil DESIGN §2 specifies — and a checkable button
// that is checked wears the Selected fill whatever role it has at rest, so a
// toggle is `checkable: true` and nothing else.
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

    // The corner of the fill. Density decides it; the icon button rounds it
    // into the suite's one hover circle.
    property real backgroundRadius: density === CelestinaButton.Prominent
                                    || density === CelestinaButton.Regular
                                    ? CelestinaTheme.radiusButton
                                    : CelestinaTheme.radiusSm

    // The role the fill and ink actually paint: a checked toggle is Selected.
    readonly property int effectiveRole: checkable && checked
                                         ? CelestinaButton.Selected : role

    // Sink on press, settle on release. Applied to fill and content, never to
    // the control: the hit box stays where the pointer found it.
    readonly property real recoil: down ? CelestinaTheme.pressRecoilScale : 1

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

    // Fast in, slow out: the sink is immediate, the return is felt. Read when
    // the Behavior starts, so `down` is the state being entered.
    readonly property int recoilDuration: CelestinaTheme.reducedMotion
                                          ? 0 : down ? CelestinaTheme.motionFast
                                                     : CelestinaTheme.motionSlow

    contentItem: Text {
        text: control.text
        // A label is text, never markup. These controls carry strings their
        // process did not write — a notification's action, another
        // application's tray menu — and `Text.AutoText` renders anything that
        // looks like markup as rich text, which lets a producer draw its own
        // interface inside ours and, with `<img src=…>`, make this process
        // fetch a URL on its behalf.
        textFormat: Text.PlainText
        font: control.font
        color: !control.enabled
               ? CelestinaTheme.textMuted
               : control.effectiveRole === CelestinaButton.Primary ? CelestinaTheme.accentInk
               : control.effectiveRole === CelestinaButton.Destructive
                 ? (control.down ? CelestinaTheme.dangerInk
                                 : CelestinaTheme.dangerFillInk)
               : control.effectiveRole === CelestinaButton.Selected ? CelestinaTheme.accentLink
               : CelestinaTheme.text
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        scale: control.recoil
        transformOrigin: Item.Center

        Behavior on scale {
            NumberAnimation {
                duration: control.recoilDuration
                easing.type: CelestinaTheme.easeStandard
            }
        }
    }

    background: Rectangle {
        radius: control.backgroundRadius
        opacity: control.enabled ? 1 : CelestinaTheme.disabledOpacity
        scale: control.recoil
        transformOrigin: Item.Center
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
                return control.effectiveRole === CelestinaButton.Primary
                     ? CelestinaTheme.accentDisabledFill
                     : CelestinaTheme.controlFill
            if (control.effectiveRole === CelestinaButton.Primary)
                return control.down ? CelestinaTheme.accentPressed
                     : control.hovered ? CelestinaTheme.accentHover
                     : CelestinaTheme.accent
            if (control.effectiveRole === CelestinaButton.Destructive)
                return control.down ? CelestinaTheme.danger
                     : control.hovered ? CelestinaTheme.dangerBorder
                     : CelestinaTheme.dangerFill
            if (control.effectiveRole === CelestinaButton.Selected)
                return control.down ? CelestinaTheme.surfaceSelected
                     : control.hovered ? CelestinaTheme.accentSoft
                     : CelestinaTheme.badgeAccentFill
            if (control.effectiveRole === CelestinaButton.Ghost)
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

        Behavior on scale {
            NumberAnimation {
                duration: control.recoilDuration
                easing.type: CelestinaTheme.easeStandard
            }
        }
    }
}
