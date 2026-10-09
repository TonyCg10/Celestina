pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0

// A Wi-Fi strength in four bars of rising height, in the glyphs' ink; the
// bars above the strength stay faint. The style has one Wi-Fi glyph with no
// levels, so the levels are drawn here.
Item {
    id: bars

    // 0–4, as the model's `bars` role reports it.
    property int level: 0

    // Four bars, the scale's smallest step apart, filling the glyph's width
    // and three quarters of its height (the box the Wi-Fi glyph's arcs fill).
    readonly property int count: 4
    readonly property int gap: CelestinaTheme.spaceXs
    readonly property real heightRatio: 0.75

    width: CelestinaTheme.iconMd
    height: CelestinaTheme.iconMd

    Row {
        id: strip
        anchors.centerIn: parent
        height: bars.height * bars.heightRatio
        spacing: bars.gap

        Repeater {
            model: bars.count

            delegate: Rectangle {
                id: bar

                required property int index

                y: strip.height - bar.height
                width: (bars.width - bars.gap * (bars.count - 1)) / bars.count
                height: strip.height * (bar.index + 1) / bars.count
                color: CelestinaTheme.textMuted
                opacity: bar.index < bars.level ? 1 : CelestinaTheme.unavailableContentOpacity
            }
        }
    }
}
