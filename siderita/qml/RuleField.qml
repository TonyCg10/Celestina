import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// ─── RuleField ────────────────────────────────────────────────────────────────
// El campo de texto de las reglas de renombrado por lotes: mismo alto y mismo
// borde que el resto de campos, con la etiqueta encima en vez de dentro.
// ──────────────────────────────────────────────────────────────────────────────
TextField {
    id: ruleField

    height: CelestinaTheme.controlHeight
    color: CelestinaTheme.text
    selectionColor: CelestinaTheme.accentStrong
    selectedTextColor: CelestinaTheme.text
    font.family: CelestinaTheme.sansFamily
    font.pixelSize: CelestinaTheme.fontBody
    leftPadding: 12
    rightPadding: 12

    background: Rectangle {
        radius: CelestinaTheme.radiusSm
        color: CelestinaTheme.inputFill
        border.width: 1
        border.color: ruleField.activeFocus ? CelestinaTheme.focus
                                            : CelestinaTheme.inputBorder
    }
}
