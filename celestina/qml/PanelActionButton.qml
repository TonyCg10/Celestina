import CelestinaStyle
import QtQuick

// A permanent icon entry point on the panel. The overlay itself remains owned
// by the host; this control only reports a click. A standalone opener owns its
// content capsule, while an opener inside a semantic PanelCluster delegates
// that material to the cluster. The complete bar owns the compositor region.
PanelMenuButton {
    id: root

    required property bool blurAvailable
    // Non-zero only from a real bar; see PanelPill.
    property real barHeight: 0
    required property string iconName
    property string fallbackIcon: iconName
    property int iconSize: CelestinaTheme.iconSm
    property bool ownsGlass: true
    attachmentAnchor: glyph

    implicitWidth: implicitHeight
    Accessible.name: helpText.length > 0 ? helpText : iconName

    contentItem: Item {
        implicitWidth: root.iconSize
        implicitHeight: root.iconSize

        CelestinaIcon {
            id: glyph

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
        barHeight: root.barHeight
        id: ownPill

        visible: root.ownsGlass
        blurAvailable: root.blurAvailable
        ink: root.ink
    }
}
