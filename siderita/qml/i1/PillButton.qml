import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// ─── PillButton ───────────────────────────────────────────────────────────────
// El botón de los diálogos, en sus tres papeles: normal, principal y
// destructivo. El destructivo no es rojo por decoración — es lo único que
// distingue "Mover a la papelera" de "Cancelar" cuando se lee deprisa.
// ──────────────────────────────────────────────────────────────────────────────
Button {
    id: pill

    property bool primary: false
    // A danger-tinted outline variant for irreversible actions (empty Trash).
    property bool destructive: false

    hoverEnabled: true
    implicitHeight: 30
    leftPadding: 14
    rightPadding: 14
    font.family: CelestinaTheme.sansFamily
    font.pixelSize: CelestinaTheme.fontLabel
    font.weight: CelestinaTheme.weightMedium

    contentItem: Text {
        text: pill.text
        font: pill.font
        color: !pill.enabled
               ? CelestinaTheme.textMuted
               : pill.primary ? CelestinaTheme.canvas
               : pill.destructive ? CelestinaTheme.dangerText
               : CelestinaTheme.text
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    background: Rectangle {
        radius: CelestinaTheme.radiusSm
        opacity: pill.enabled ? 1 : 0.5
        color: {
            if (!pill.enabled)
                return CelestinaTheme.controlFill
            if (pill.primary)
                return pill.down ? Qt.darker(CelestinaTheme.accent, 1.18)
                     : pill.hovered ? Qt.darker(CelestinaTheme.accent, 1.08)
                     : CelestinaTheme.accent
            if (pill.destructive)
                return pill.down ? CelestinaTheme.danger
                     : pill.hovered ? CelestinaTheme.dangerBorder
                     : CelestinaTheme.dangerFill
            return pill.down ? CelestinaTheme.surfaceStrong
                 : pill.hovered ? CelestinaTheme.surfaceHover
                 : CelestinaTheme.controlFill
        }
        border.width: pill.primary ? 0 : 1
        border.color: pill.activeFocus ? CelestinaTheme.focus
                      : pill.destructive ? CelestinaTheme.dangerBorder
                      : CelestinaTheme.border

        Behavior on color {
            ColorAnimation { duration: CelestinaTheme.motionFast }
        }
    }
}
