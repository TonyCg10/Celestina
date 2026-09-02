import QtQuick
import QtQuick.Controls

// Icon-only specialization of the shared button. It inherits the same role,
// density, focus and disabled contracts rather than rebuilding a ToolButton.
CelestinaButton {
    id: control

    required property string iconName
    property string fallbackIcon: iconName
    property int iconSize: CelestinaTheme.iconSm

    // Square by default, at the density's control height. Nothing here pins a
    // consumer to that size in either direction: the background is the shared
    // button's and always fills the control, and Ghost paints nothing at rest,
    // so a consumer that wants a smaller *visual* keeps the control — and its
    // hit box — at the `controlHeightXs` floor and shrinks `iconSize` alone,
    // rather than shrinking the control under the floor.
    implicitWidth: implicitHeight
    leftPadding: 0
    rightPadding: 0
    topPadding: 0
    bottomPadding: 0
    // An icon-only button carries no text, so `helpText` is the only name a
    // screen reader would ever hear — and it defaults to empty. Degrade to the
    // icon's semantic name rather than announcing an anonymous button.
    //
    // Degrade rather than require: `helpText` is inherited from
    // `CelestinaButton`, where it is genuinely optional because a labelled
    // button already has its text, and making it required here would refuse to
    // construct the majority of the icon buttons the suite has today. That is a
    // consumer-by-consumer copy pass, not this control's fix. `iconName` is an
    // English catalogue key and therefore a placeholder, not product copy: it
    // keeps the control operable and makes the missing label audible instead of
    // silent. The same degradation Siderita's floating button already applies.
    Accessible.name: helpText.length > 0 ? helpText : iconName
    display: AbstractButton.IconOnly

    // Controls own their contentItem's rectangle. Keep that layout viewport
    // separate from the glyph so a button's padding/density can never turn a
    // square icon into a wide or tall raster.
    contentItem: Item {
        implicitWidth: control.iconSize
        implicitHeight: control.iconSize

        CelestinaIcon {
            name: control.iconName
            fallbackName: control.fallbackIcon
            width: Math.max(1, Math.min(control.iconSize,
                                        parent.width, parent.height))
            height: width
            anchors.centerIn: parent
            tone: !control.enabled
                  ? CelestinaIcon.Secondary
                  : control.role === CelestinaButton.Primary
                    ? CelestinaIcon.OnAccent
                  : control.role === CelestinaButton.Selected
                    ? CelestinaIcon.Accent
                    : CelestinaIcon.Primary
        }
    }
}
