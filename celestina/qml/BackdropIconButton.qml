// Icon-only specialization of BackdropButton.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Controls

BackdropButton {
    id: control

    required property string iconName
    property string fallbackIcon: iconName
    property int iconSize: CelestinaTheme.iconSm

    implicitWidth: implicitHeight
    leftPadding: 0
    rightPadding: 0
    topPadding: 0
    bottomPadding: 0
    Accessible.name: helpText.length > 0 ? helpText : iconName
    display: AbstractButton.IconOnly

    contentItem: Item {
        implicitWidth: control.iconSize
        implicitHeight: control.iconSize

        CelestinaIcon {
            anchors.centerIn: parent
            width: Math.max(1, Math.min(control.iconSize,
                                        parent.width, parent.height))
            height: width
            name: control.iconName
            fallbackName: control.fallbackIcon
            tintOverride: !control.enabled ? control.ink.muted
                          : control.role === CelestinaButton.Primary
                            ? CelestinaTheme.accentInk
                          : control.role === CelestinaButton.Destructive
                            ? (control.down ? CelestinaTheme.dangerInk
                                            : CelestinaTheme.dangerFillInk)
                          : control.role === CelestinaButton.Selected
                            ? control.ink.accent : control.ink.primary
            Accessible.ignored: true
        }
    }
}
