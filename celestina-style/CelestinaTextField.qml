import QtQuick
import QtQuick.Controls

// ─── CelestinaTextField ───────────────────────────────────────────────────────
// El campo de texto del suite: color, selección, familia y el fondo temático
// (relleno que se aclara al foco + borde de foco) en un solo sitio, en vez de
// repetir el mismo `background: Rectangle { inputFill … }` en cada diálogo.
//
// El tamaño se deja al consumidor —alto, tamaño de letra, `radius`, padding—
// porque un renombrado en línea es compacto y un campo de diálogo no; lo que se
// comparte es el vestido, no las medidas.
// ──────────────────────────────────────────────────────────────────────────────
TextField {
    id: field

    // El consumidor lo baja para un campo compacto (radiusXs) sin tocar el resto.
    property int radius: CelestinaTheme.radiusSm

    height: CelestinaTheme.controlHeight
    color: CelestinaTheme.text
    selectionColor: CelestinaTheme.accentStrong
    selectedTextColor: CelestinaTheme.text
    placeholderTextColor: CelestinaTheme.textMuted
    font.family: CelestinaTheme.sansFamily
    font.pixelSize: CelestinaTheme.fontBody
    leftPadding: 12
    rightPadding: 12

    background: Rectangle {
        radius: field.radius
        color: field.activeFocus ? CelestinaTheme.inputFillFocus
                                 : CelestinaTheme.inputFill
        border.width: 1
        border.color: field.activeFocus ? CelestinaTheme.focus
                                        : CelestinaTheme.inputBorder

        Behavior on color {
            ColorAnimation { duration: CelestinaTheme.motionFast }
        }
    }
}
