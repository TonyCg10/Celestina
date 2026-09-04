import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// Siderita-only button for controls floating directly over scrollable content.
// The suite button owns ordinary solid actions; this specialization owns the
// injected backdrop and keeps the same closed tonal/primary vocabulary.
//
// Icon-first: a consumer that sets `iconName` gets a glyph, and with no text
// the pill collapses to a circle — the same hover shape as every other icon
// action in the suite. `helpText` then carries the words as tooltip and
// accessible name.
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
    property string iconName: ""
    property string fallbackIcon: iconName

    readonly property bool iconOnly: iconName.length > 0 && text.length === 0
    // Sink on press, settle on release. Applied to fill and content, never to
    // the control: the hit box stays where the pointer found it.
    readonly property real recoil: down ? CelestinaTheme.pressRecoilScale : 1

    hoverEnabled: true
    implicitHeight: CelestinaTheme.controlHeightSm
    implicitWidth: iconOnly
                   ? implicitHeight
                   : implicitContentWidth + leftPadding + rightPadding
    leftPadding: iconOnly ? 0 : CelestinaTheme.compButtonPaddingHorizontal
                                + CelestinaTheme.spaceXs
    rightPadding: iconOnly ? 0 : CelestinaTheme.compButtonPaddingHorizontal
                                 + CelestinaTheme.spaceXs
    font.family: CelestinaTheme.sansFamily
    font.pixelSize: CelestinaTheme.fontRowSecondary
    font.weight: CelestinaTheme.weightMedium

    Accessible.name: helpText.length > 0 ? helpText : text
    ToolTip.visible: helpText.length > 0 && shield.hovered
    ToolTip.text: helpText

    // The button already owns its click — a swallowing MouseArea would be
    // delivered before it — but hover and drag still leak through to the
    // delegate underneath unless something claims them. The shield's blocking
    // hover also takes the hover from this Button, so the fill below reads the
    // shield's `hovered` instead of `control.hovered`, which would never rise.
    // It yields the drag grab: the Button is the press owner here.
    CelestinaInputShield {
        id: shield
        swallowClicks: false
        yieldsToHost: true
    }

    // Fast in, slow out: the sink is immediate, the return is felt.
    readonly property int recoilDuration: CelestinaTheme.reducedMotion
                                          ? 0 : down ? CelestinaTheme.motionFast
                                                     : CelestinaTheme.motionSlow

    contentItem: Item {
        implicitWidth: contentRow.implicitWidth
        implicitHeight: contentRow.implicitHeight
        scale: control.recoil
        transformOrigin: Item.Center

        Behavior on scale {
            NumberAnimation {
                duration: control.recoilDuration
                easing.type: CelestinaTheme.easeStandard
            }
        }

        Row {
            id: contentRow
            anchors.centerIn: parent
            spacing: CelestinaTheme.spaceXs

            CelestinaIcon {
                visible: control.iconName.length > 0
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.iconSm
                height: width
                name: control.iconName
                fallbackName: control.fallbackIcon
                tone: !control.enabled
                      ? CelestinaIcon.Secondary
                      : control.role === FloatingButton.Primary
                        ? CelestinaIcon.OnAccent
                      : control.active
                        ? CelestinaIcon.Accent
                        : CelestinaIcon.Primary
            }

            Text {
                visible: control.text.length > 0
                anchors.verticalCenter: parent.verticalCenter
                text: control.text
                textFormat: Text.PlainText
                font: control.font
                color: control.role === FloatingButton.Primary
                       ? (control.enabled ? CelestinaTheme.accentInk
                                          : CelestinaTheme.accentDisabledInk)
                       : (control.enabled ? (control.active ? CelestinaTheme.accent
                                                             : CelestinaTheme.text)
                                          : CelestinaTheme.textMuted)
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
            }
        }
    }

    background: GlassPill {
        inputShield: false
        backdrop: control.backdrop
        floating: control.floating
        scale: control.recoil
        transformOrigin: Item.Center
        // Three distinct states: rest, hover, active — and press darkest. An
        // active pill that merely looked hovered gave no clue it was on.
        fill: control.role === FloatingButton.Primary
              ? (!control.enabled ? CelestinaTheme.accentDisabledFill
                 : control.down ? CelestinaTheme.accentPressed
                 : shield.hovered ? CelestinaTheme.accentHover
                 : CelestinaTheme.accent)
              : (!control.enabled ? CelestinaTheme.controlFill
                 : control.down ? CelestinaTheme.surfaceStrong
                 : shield.hovered ? CelestinaTheme.surfaceHover
                 : control.active ? CelestinaTheme.badgeAccentFill
                 : CelestinaTheme.controlFill)
        border.width: control.role === FloatingButton.Primary && !control.enabled
                        ? CelestinaTheme.borderHairline
                      : control.role === FloatingButton.Tonal
                        ? CelestinaTheme.borderHairline : 0
        border.color: control.role === FloatingButton.Primary
                        ? CelestinaTheme.accentDisabledInk
                        : CelestinaTheme.divider

        Behavior on scale {
            NumberAnimation {
                duration: control.recoilDuration
                easing.type: CelestinaTheme.easeStandard
            }
        }

        CelestinaFocusRing {
            target: parent
            cornerRadius: parent.radius
            shown: control.visualFocus
        }
    }
}
