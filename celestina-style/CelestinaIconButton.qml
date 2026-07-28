import QtQuick
import QtQuick.Controls

// Icon-only specialization of the shared button. It inherits the same role,
// density, focus and disabled contracts rather than rebuilding a ToolButton.
CelestinaButton {
    id: control

    required property string iconName
    property string fallbackIcon: iconName

    implicitWidth: implicitHeight
    leftPadding: 0
    rightPadding: 0
    Accessible.name: helpText
    display: AbstractButton.IconOnly

    contentItem: CelestinaIcon {
        name: control.iconName
        fallbackName: control.fallbackIcon
        width: CelestinaTheme.iconSm
        height: CelestinaTheme.iconSm
        anchors.centerIn: parent
        tone: !control.enabled
              ? CelestinaIcon.Secondary
              : control.role === CelestinaButton.Primary
                ? CelestinaIcon.OnAccent
                : CelestinaIcon.Primary
    }
}
