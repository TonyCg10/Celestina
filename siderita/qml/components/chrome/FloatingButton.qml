import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// Siderita-only button for controls floating directly over scrollable content.
// The suite button owns ordinary solid actions; this specialization owns the
// injected backdrop and keeps the same closed tonal/primary vocabulary.
Button {
    id: control

    enum Role {
        Tonal,
        Primary
    }

    property int role: FloatingButton.Tonal
    property Item backdrop
    property bool floating: false
    property bool active: false
    property string helpText: ""

    hoverEnabled: true
    implicitHeight: CelestinaTheme.controlHeightSm
    leftPadding: CelestinaTheme.compButtonPaddingHorizontal
                 + CelestinaTheme.spaceXs
    rightPadding: CelestinaTheme.compButtonPaddingHorizontal
                  + CelestinaTheme.spaceXs
    font.family: CelestinaTheme.sansFamily
    font.pixelSize: CelestinaTheme.fontRowSecondary
    font.weight: CelestinaTheme.weightMedium

    ToolTip.visible: helpText.length > 0 && hovered
    ToolTip.text: helpText
    Accessible.name: helpText.length > 0 ? helpText : text

    // QQuickControl handles clicks, while this handler prevents hover from
    // leaking through the translucent background to a file delegate.
    HoverHandler {
        blocking: true
    }

    // A drag beginning on a button belongs to the floating chrome, never to
    // the file row/cell beneath it.
    DragHandler {
        target: null
        grabPermissions: PointerHandler.CanTakeOverFromAnything
                         | PointerHandler.ApprovesTakeOverByAnything
    }

    contentItem: Text {
        text: control.text
        font: control.font
        color: control.role === FloatingButton.Primary
               ? (control.enabled ? CelestinaTheme.accentInk
                                  : CelestinaTheme.accentDisabledInk)
               : (control.enabled ? CelestinaTheme.text
                                  : CelestinaTheme.textMuted)
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }

    background: GlassPill {
        inputShield: false
        backdrop: control.backdrop
        floating: control.floating
        fill: control.role === FloatingButton.Primary
              ? (!control.enabled ? CelestinaTheme.accentDisabledFill
                 : control.down ? CelestinaTheme.accentPressed
                 : control.hovered ? CelestinaTheme.accentHover
                 : CelestinaTheme.accent)
              : (!control.enabled ? CelestinaTheme.controlFill
                 : control.down ? CelestinaTheme.surfaceStrong
                 : control.hovered || control.active ? CelestinaTheme.surfaceHover
                 : CelestinaTheme.controlFill)
        border.width: control.activeFocus ? CelestinaTheme.borderFocus
                      : control.role === FloatingButton.Primary && !control.enabled
                        ? CelestinaTheme.borderHairline
                      : control.role === FloatingButton.Tonal
                        ? CelestinaTheme.borderHairline : 0
        border.color: control.activeFocus ? CelestinaTheme.focusRing
                      : control.role === FloatingButton.Primary
                        ? CelestinaTheme.accentDisabledInk
                        : CelestinaTheme.divider
    }
}
