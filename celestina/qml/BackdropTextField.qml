// Text input for a transparent PANEL-1 menu field.
//
// The suite control owns editing, selection and keyboard behavior. This local
// specialization replaces only its opaque scheme-bound plate with the same
// low-density fixed-ink material used by the surrounding menu.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

CelestinaTextField {
    id: field

    required property BackdropInk ink

    color: field.ink.primary
    placeholderTextColor: field.ink.muted
    selectionColor: field.ink.selectedFill
    selectedTextColor: field.ink.primary

    background: Rectangle {
        radius: field.fieldRadius
        color: field.visualFocus ? field.ink.selectedRestFill
                                 : field.ink.materialTint
        opacity: field.visualFocus
                 ? CelestinaTheme.decorationOpacitySoft / 2
                 : CelestinaTheme.decorationOpacitySoft / 4
        border.width: field.visualFocus ? CelestinaTheme.borderFocus
                                        : CelestinaTheme.borderHairline
        border.color: field.visualFocus ? field.ink.focus : field.ink.divider

        Behavior on color {
            enabled: !CelestinaTheme.reducedMotion

            ColorAnimation {
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }
    }
}
