import QtQuick
import org.celestina.hematita 1.0

// The centred pill that names the sections. Built on the suite's capsule so
// the resting fill and the pill radius are the shared ones; the items inside
// are Hematita's, because a glyph-over-word destination with a lit current
// state is not a control the style ships yet. It stays here until a second
// application needs it.
//
// Keyboard: one radio group. Left/Right move the selection, Tab leaves.
FocusScope {
    id: strip

    // Each entry: { key: string, icon: string, label: string }.
    required property var model
    property int currentIndex: 0

    signal activated(int index)

    implicitWidth: capsule.implicitWidth
    implicitHeight: capsule.implicitHeight

    Accessible.role: Accessible.PageTabList

    function move(delta) {
        const count = strip.model.length
        if (count === 0)
            return
        const next = (strip.currentIndex + delta + count) % count
        strip.activated(next)
    }

    Keys.onLeftPressed: function(event) { strip.move(-1); event.accepted = true }
    Keys.onRightPressed: function(event) { strip.move(1); event.accepted = true }

    CelestinaCapsule {
        id: capsule
        inset: CelestinaTheme.spaceXs
        spacing: CelestinaTheme.spaceXs

        Repeater {
            model: strip.model

            NavItem {
                required property int index
                required property var modelData

                iconName: modelData.icon
                label: modelData.label
                current: index === strip.currentIndex
                // Only the current item is in the Tab order; arrows walk
                // the rest, so Tab lands on the strip once and leaves once.
                focus: current
                onClicked: strip.activated(index)
            }
        }
    }
}
