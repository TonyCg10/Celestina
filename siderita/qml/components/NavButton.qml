import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import org.celestina.siderita 1.0

// ─── NavButton ────────────────────────────────────────────────────────────────
// Un botón de sólo icono con su ayuda emergente: navegación, nueva pestaña,
// dirección de orden. Sin texto porque va en una barra donde el espacio es del
// contenido, con tooltip porque un icono sin nombre es una adivinanza.
// ──────────────────────────────────────────────────────────────────────────────
ToolButton {
    id: control

    required property string iconName
    required property string fallbackIcon
    required property string helpText

    implicitWidth: CelestinaTheme.controlHeight
    implicitHeight: CelestinaTheme.controlHeight
    hoverEnabled: true
    ToolTip.visible: hovered
    ToolTip.text: helpText
    ToolTip.delay: 550
    Accessible.name: helpText
    display: AbstractButton.IconOnly
    icon.name: iconName
    icon.source: CelestinaTheme.fallbackIcon(fallbackIcon)
    icon.width: CelestinaTheme.iconSm
    icon.height: CelestinaTheme.iconSm
    icon.color: control.enabled
                ? CelestinaTheme.text
                : CelestinaTheme.textMuted

    background: Rectangle {
        radius: CelestinaTheme.radiusSm
        color: control.hovered
               ? CelestinaTheme.surfaceHover
               : CelestinaTheme.surface
        border.width: control.activeFocus ? 1 : 0
        border.color: CelestinaTheme.focusRing

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.motionFast
            }
        }
    }
}
