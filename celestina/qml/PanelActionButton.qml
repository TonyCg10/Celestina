import CelestinaStyle
import QtQuick.Controls

// A permanent icon entry point on the panel. The overlay itself remains owned
// by the host; this control only reports a click and contributes one glass
// region like every other panel reading.
CelestinaIconButton {
    id: root

    required property bool blurAvailable
    signal blurRegionChanged()

    height: CelestinaTheme.controlHeightXs
    iconSize: CelestinaTheme.iconSm
    role: CelestinaButton.Ghost
    ToolTip.visible: false

    PanelPill {
        blurAvailable: root.blurAvailable
        onBlurRegionChanged: root.blurRegionChanged()
    }
}
