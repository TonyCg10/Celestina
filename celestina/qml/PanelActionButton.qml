import CelestinaStyle
import QtQuick

// A permanent icon entry point on the panel. The overlay itself remains owned
// by the host; this control only reports a click. A standalone opener owns its
// glass region, while an opener inside a semantic PanelCluster delegates that
// material to the cluster so one group never publishes overlapping blur.
PanelMenuButton {
    id: root

    required property bool blurAvailable
    required property string iconName
    property string fallbackIcon: iconName
    property int iconSize: CelestinaTheme.iconSm
    property bool ownsGlass: true
    signal blurRegionChanged()

    implicitWidth: implicitHeight
    Accessible.name: helpText.length > 0 ? helpText : iconName

    contentItem: Item {
        implicitWidth: root.iconSize
        implicitHeight: root.iconSize

        CelestinaIcon {
            anchors.centerIn: parent
            width: Math.max(1, Math.min(root.iconSize,
                                        parent.width, parent.height))
            height: width
            name: root.iconName
            fallbackName: root.fallbackIcon
            tone: root.enabled ? CelestinaIcon.Primary
                               : CelestinaIcon.Secondary
            tintOverride: root.enabled ? root.ink.primary : root.ink.muted
            Accessible.ignored: true
        }
    }

    PanelPill {
        visible: root.ownsGlass
        blurAvailable: root.blurAvailable
        ink: root.ink
        onBlurRegionChanged: root.blurRegionChanged()
    }
}
