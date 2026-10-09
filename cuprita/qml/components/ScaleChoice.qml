pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.celestina.cuprita 1.0

// A choice among a few words in one control-shaped plate, drawn like the
// suite's CelestinaSegmentedControl (the plate `radiusButton` on `card`, the
// chosen segment `surfaceSelected` `spaceXs` inside it) but without icons.
// The shared control is not used: its delegate reaches outer ids without
// `pragma ComponentBehavior: Bound`, which would grow Cuprita's qmllint
// ratchet by five, and celestina-style is outside this application's scope.
//
// Stateless like it: the control never moves `currentIndex` itself; it asks
// through `activated(index)` and the host answers. One Tab stop; Left/Right
// walk the segments (wrapping), Home/End reach the ends. The current segment
// holds the focus, so a screen reader announces the chosen word. Narrower
// than its words, the control shares its width evenly and elides them.
FocusScope {
    id: control

    activeFocusOnTab: true

    // [{ key: string, label: string }]
    required property var model
    property int currentIndex: 0
    // The name a screen reader hears for the whole choice.
    property string helpText: ""

    signal activated(int index)

    // The segments' natural widths, not the Row's: a shrunk segment must not
    // shrink what the control asks for.
    implicitWidth: {
        let total = CelestinaTheme.spaceXs * 2 + row.spacing * Math.max(0, segments.count - 1)
        for (let i = 0; i < segments.count; ++i) {
            const item = segments.itemAt(i)
            if (item)
                total += item.implicitWidth
        }
        return total
    }
    implicitHeight: CelestinaTheme.compSegmentHeight

    Accessible.role: Accessible.PageTabList
    Accessible.name: control.helpText

    function move(delta) {
        const count = control.model.length
        if (count === 0)
            return
        control.activated((control.currentIndex + delta + count) % count)
    }

    Keys.onLeftPressed: function(event) { control.move(-1); event.accepted = true }
    Keys.onRightPressed: function(event) { control.move(1); event.accepted = true }
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Home && control.model.length > 0) {
            control.activated(0)
            event.accepted = true
        } else if (event.key === Qt.Key_End && control.model.length > 0) {
            control.activated(control.model.length - 1)
            event.accepted = true
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: CelestinaTheme.radiusButton
        color: CelestinaTheme.card
    }

    Row {
        id: row
        anchors.fill: parent
        anchors.margins: CelestinaTheme.spaceXs
        // The plate inset plus the gap, so each seam reads `spaceXs` a side.
        spacing: CelestinaTheme.spaceXs * 2

        Repeater {
            id: segments
            model: control.model

            AbstractButton {
                id: segment

                required property int index
                required property var modelData
                readonly property bool current: index === control.currentIndex

                objectName: "segment-" + index
                height: row.height
                implicitWidth: label.implicitWidth + CelestinaTheme.spaceMd * 2
                width: control.width >= control.implicitWidth
                       ? implicitWidth
                       : (row.width - row.spacing * (control.model.length - 1)) / control.model.length
                hoverEnabled: true
                // Only the current segment is in the Tab order.
                focusPolicy: segment.current ? Qt.TabFocus : Qt.NoFocus
                focus: segment.current

                Accessible.role: Accessible.PageTab
                Accessible.name: segment.modelData.label
                Accessible.checked: segment.current
                Accessible.selected: segment.current

                onClicked: control.activated(segment.index)

                background: Rectangle {
                    radius: CelestinaTheme.radiusButton - CelestinaTheme.spaceXs
                    color: segment.current
                           ? CelestinaTheme.surfaceSelected
                           : segment.down
                             ? CelestinaTheme.pressedWash
                             : segment.hovered
                               ? CelestinaTheme.surfaceHover
                               : CelestinaTheme.clear
                    Behavior on color {
                        ColorAnimation {
                            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionFast
                        }
                    }
                }

                CelestinaFocusRing {
                    target: segment
                    cornerRadius: CelestinaTheme.radiusButton - CelestinaTheme.spaceXs
                    shown: segment.visualFocus
                }

                contentItem: Text {
                    id: label
                    text: segment.modelData.label
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                    font.weight: segment.current ? CelestinaTheme.weightDemiBold : CelestinaTheme.weightRegular
                    color: segment.current ? CelestinaTheme.text : CelestinaTheme.textMuted
                }
            }
        }
    }
}
