// One semantic group on the panel, with compact internal rhythm and exactly
// one dense content capsule. The panel backdrop owns the sole compositor-glass
// region; controls inside this row use the same spacing as the Wi-Fi and
// Bluetooth pair.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: root

    required property bool blurAvailable
    required property BackdropInk ink
    // Non-zero only from a real bar; see PanelPill.
    property real barHeight: 0
    property int spacing: CelestinaTheme.spaceMd
    default property alias controls: controls.data
    // A caller with late data may bind this to the model-backed presence bit
    // rather than to effective child visibility. The natural row width is the
    // safe default for groups whose controls are permanent.
    property bool hasContent: controls.implicitWidth > 0
    // Whether the glass floor is painted. It follows presence for every ordinary group;
    // a group that reserves space for something invisible separates the two, so the space
    // can exist without a pill sitting under nothing.
    property bool showsGlass: root.hasContent

    implicitWidth: root.hasContent ? controls.implicitWidth : 0
    implicitHeight: CelestinaTheme.controlHeightXs
    visible: root.hasContent

    Row {
        id: controls

        anchors.centerIn: parent
        spacing: root.spacing
    }

    PanelPill {
        barHeight: root.barHeight
        id: pill

        visible: root.showsGlass
        blurAvailable: root.blurAvailable
        ink: root.ink
    }
}
