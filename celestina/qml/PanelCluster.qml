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
    property int spacing: CelestinaTheme.spaceMd
    default property alias controls: controls.data
    // A caller with late data may bind this to the model-backed presence bit
    // rather than to effective child visibility. The natural row width is the
    // safe default for groups whose controls are permanent.
    property bool hasContent: controls.implicitWidth > 0

    implicitWidth: root.hasContent ? controls.implicitWidth : 0
    implicitHeight: CelestinaTheme.controlHeightXs
    visible: root.hasContent

    Row {
        id: controls

        anchors.centerIn: parent
        spacing: root.spacing
    }

    PanelPill {
        id: pill

        visible: root.hasContent
        blurAvailable: root.blurAvailable
        ink: root.ink
    }
}
