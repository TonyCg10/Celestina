import QtQuick
import QtQuick.Controls

// Icon-only specialization of the shared button. It inherits the same role,
// density, focus and disabled contracts rather than rebuilding a ToolButton.
CelestinaButton {
    id: control

    required property string iconName
    property string fallbackIcon: iconName
    property int iconSize: CelestinaTheme.iconSm

    implicitWidth: implicitHeight
    leftPadding: 0
    rightPadding: 0
    topPadding: 0
    bottomPadding: 0
    Accessible.name: helpText
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
