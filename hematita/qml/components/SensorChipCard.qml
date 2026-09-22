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
    // A channel-shaped nothing, so a row whose index is momentarily past the
    // array reads fields rather than undefined.
    readonly property var emptyChannel: ({ label: "", valueText: "", extremesText: "",
                                           limitText: "", load: "normal" })

    title: card.chipTitle
    // The page is the one Tab stop; a card is a place in it, not a stop.
    activeFocusOnTab: false

    // The model is the channel count, not the array: a chip publishes the same
    // channels every tick, and a fresh array as the model would tear every row
    // down and build it again each second — taking the keyboard focus with it.
    // The rows read this tick's values by index instead, as the outer card
    // repeater and the process tables do.
    Repeater {
        model: card.channels.length

        SensorRow {
            required property int index
            readonly property var channel: index < card.channels.length
                                           ? card.channels[index] : card.emptyChannel

            width: card.width
            label: channel.label
            valueText: channel.valueText
            extremesText: channel.extremesText
            limitText: channel.limitText
            load: channel.load
        }
    }
}
