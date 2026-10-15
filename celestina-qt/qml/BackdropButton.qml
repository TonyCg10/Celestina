// A shell-local button whose neutral ink follows the shell's fixed light
// foreground contract.
// Filled primary and destructive roles retain their canonical surface/ink pair.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Controls

CelestinaButton {
    id: control

    required property BackdropInk ink
    // A transient surface may keep its opener identifiable after the pointer
    // leaves the panel. This reuses the exact hover layer; it does not invent a
    // selected role or alter the button's geometry.
    property bool holdHoverFeedback: false
    readonly property bool hoverFeedbackActive:
            control.hovered || control.holdHoverFeedback

    // Shell controls keep their concise names for AT-SPI, but never paint
    // hover cards above the compact panel or its transient surfaces. Override
    // both attached bindings owned by CelestinaButton so every shell-local
    // specialization inherits the same no-tooltip contract.
    ToolTip.visible: false
    ToolTip.text: ""

    contentItem: Text {
        text: control.text
        textFormat: Text.PlainText
        font: control.font
        color: !control.enabled ? control.ink.muted
               : control.role === CelestinaButton.Primary
                 ? CelestinaTheme.accentInk
               : control.role === CelestinaButton.Destructive
                 ? (control.down ? CelestinaTheme.dangerInk
                                 : CelestinaTheme.dangerFillInk)
               : control.role === CelestinaButton.Selected
                 ? control.ink.accent : control.ink.primary
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    background: Rectangle {
        id: buttonBackground

        radius: control.density === CelestinaButton.Prominent
                || control.density === CelestinaButton.Regular
                ? CelestinaTheme.radiusButton : CelestinaTheme.radiusSm
        opacity: control.enabled ? 1 : CelestinaTheme.disabledOpacity
        color: {
            if (!control.enabled)
                return control.role === CelestinaButton.Primary
                     ? CelestinaTheme.accentDisabledFill
                     : control.ink.controlFill;
            if (control.role === CelestinaButton.Primary)
                return control.down ? CelestinaTheme.accentPressed
                     : control.hoverFeedbackActive ? CelestinaTheme.accentHover
                     : CelestinaTheme.accent;
            if (control.role === CelestinaButton.Destructive)
                return control.down ? CelestinaTheme.danger
                     : control.hoverFeedbackActive ? CelestinaTheme.dangerBorder
                     : CelestinaTheme.dangerFill;
            if (control.role === CelestinaButton.Selected)
                return control.down ? control.ink.selectedFill
                     : control.hoverFeedbackActive ? control.ink.accentFill
                     : control.ink.selectedRestFill;
            if (control.role === CelestinaButton.Ghost)
                return control.down ? control.ink.pressedFill
                     : control.hoverFeedbackActive ? control.ink.controlFill
                     : CelestinaTheme.clear;
            return control.down ? control.ink.pressedFill
                 : control.hoverFeedbackActive ? control.ink.hoverFill
                 : control.ink.controlFill;
        }
        border.width: 0

        Rectangle {
            anchors.fill: parent
            anchors.margins: -CelestinaTheme.borderFocus
            radius: parent.radius + CelestinaTheme.borderFocus
            color: CelestinaTheme.clear
            border.width: CelestinaTheme.borderFocus
            border.color: control.ink.focus
            visible: control.visualFocus
            z: 1000
            Accessible.ignored: true
        }

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
            }
        }
    }
}
