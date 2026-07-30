import QtQuick
import QtQuick.Controls

// ─── CelestinaTextField ───────────────────────────────────────────────────────
// El campo de texto del suite: color, selección, familia y el fondo temático
// (relleno que se aclara al foco + anillo exterior) en un solo sitio, en vez de
// repetir el mismo `background: Rectangle { inputFill … }` en cada diálogo.
//
// El tamaño se deja al consumidor porque un renombrado en línea es compacto y
// un campo de diálogo no. La forma, en cambio, es parte del sistema visual y se
// elige con un papel cerrado en vez de un radio arbitrario.
// ──────────────────────────────────────────────────────────────────────────────
TextField {
    id: field

    enum Shape {
        Standard,
        Search
    }

    property int shape: CelestinaTextField.Standard
    readonly property real fieldRadius: shape === CelestinaTextField.Search
                                        ? CelestinaTheme.radiusInput
                                        : CelestinaTheme.radiusSm
    // Unlike Control-derived buttons, Qt's TextField is a TextInput template
    // and does not expose `visualFocus`. Mirror Qt Controls' definition so a
    // pointer click never paints the keyboard-focus ring.
    readonly property bool visualFocus: activeFocus
                                        && (focusReason === Qt.TabFocusReason
                                            || focusReason === Qt.BacktabFocusReason
                                            || focusReason === Qt.ShortcutFocusReason)

    implicitHeight: CelestinaTheme.controlHeight
    color: CelestinaTheme.text
    selectionColor: CelestinaTheme.accentPressed
    selectedTextColor: CelestinaTheme.accentInk
    placeholderTextColor: CelestinaTheme.textMuted
    font.family: CelestinaTheme.sansFamily
    font.pixelSize: CelestinaTheme.fontBody
    leftPadding: CelestinaTheme.compTextFieldPaddingHorizontal
    rightPadding: CelestinaTheme.compTextFieldPaddingHorizontal

    background: Rectangle {
        radius: field.fieldRadius
        color: field.visualFocus ? CelestinaTheme.inputFillFocus
                                 : CelestinaTheme.inputFill
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.inputBorder

        CelestinaFocusRing {
            target: parent
            cornerRadius: parent.radius
            shown: field.visualFocus
        }

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
            }
        }
    }
}
