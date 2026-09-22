pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.hematita 1.0

// One chip as a grouped card: the suite's list section with its uppercase
// eyebrow naming the chip, and one row per channel.
//
// Two names differ from the obvious ones because `ListSection` already owns
// them: the eyebrow arrives as `chipTitle` (its `title`) and the channel data
// as `channels` (its `rows`, which is its default property and holds the
// delegates the `Repeater` creates). An inherited property can be neither
// redeclared nor made `required`.
ListSection {
    id: card

    required property string chipTitle
    // { label, valueText, extremesText, limitText, load } per channel.
    required property var channels

    title: card.chipTitle

    Repeater {
        model: card.channels

        SensorRow {
            required property var modelData
            width: card.width
            label: modelData.label
            valueText: modelData.valueText
            extremesText: modelData.extremesText
            limitText: modelData.limitText
            load: modelData.load
        }
    }
}
