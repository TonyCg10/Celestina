import QtQuick
import QtQuick.Controls

// ─── CelestinaSegmentedControl ────────────────────────────────────────────────
// Peer destinations in one control-shaped plate: the sections of a resource
// monitor, the views of a library, the documents of an editor. It replaces the
// floating tab pill of the phone with the segmented control a desktop bar
// carries at its left. The plate is `radiusButton` on `card`; the current
// segment wears `surfaceSelected` at `radiusButton - spaceXs`, concentric with
// the plate because it sits `spaceXs` inside it.
//
// Navigation, not choice: activating is immediate and the control never moves
// `currentIndex` itself — it asks through `activated(index)` and the host, who
// owns the page, answers by setting `currentIndex`. One Tab stop: Tab lands on
// the current segment, Left/Right/Home/End walk the rest, Tab leaves.
//
// Model: [{ key: string, icon: string, label: string }]. `iconOnly` hides the
// words and keeps them as the segments' accessible names.
// ──────────────────────────────────────────────────────────────────────────────
FocusScope {
    id: control

    activeFocusOnTab: true

    // A FocusScope is a Tab boundary: Tab only walks *inside* it once
    // something inside already holds focus. Make the scope itself the one
    // reachable Tab stop; it then forwards active focus to whichever segment
    // asked for it (the current one), giving the control exactly one stop.

    required property var model
    property int currentIndex: 0
    property bool iconOnly: false
    // The name a screen reader hears for the whole list; product copy.
    property string helpText: ""

    signal activated(int index)

    implicitWidth: row.implicitWidth + CelestinaTheme.spaceXs * 2
    implicitHeight: CelestinaTheme.compSegmentHeight

    Accessible.role: Accessible.PageTabList
    Accessible.name: control.helpText

    // Stateless: every step is computed from the host-owned `currentIndex`,
    // never from anything the control remembers itself. A host that ignores
    // `activated()` sees the same step offered again; a host that echoes it
    // back into `currentIndex` (the normal case) keeps walking from there.
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
        id: plate
        objectName: "segmentedPlate"
        anchors.fill: parent
        radius: CelestinaTheme.radiusButton
        color: CelestinaTheme.card
    }

    Row {
        id: row
        anchors.fill: parent
        anchors.margins: CelestinaTheme.spaceXs
        // `Row`'s positioned x is relative to the Row itself, not to the
        // plate; the Row's own `spaceXs` inset from the plate edge never
        // shows up in a child's `x`. The gap between two segments must still
        // read as `spaceXs` on each side of the seam (the plate inset plus
        // the inter-segment gap), so the inter-segment spacing is doubled
        // here to carry both halves.
        spacing: CelestinaTheme.spaceXs * 2

        Repeater {
            model: control.model

            AbstractButton {
                id: segment

                required property int index
                required property var modelData
                readonly property bool current: index === control.currentIndex

                objectName: "segment-" + index
                height: row.height
                // A Row's own implicitWidth sums its children's
                // implicitWidth, not their real `width` — it still counts a
                // hidden label's natural width. Measure the content by hand
                // so `iconOnly` really shrinks the segment.
                implicitWidth: CelestinaTheme.iconMd
                               + (control.iconOnly ? 0 : CelestinaTheme.spaceSm + labelText.implicitWidth)
                               + CelestinaTheme.spaceMd * 2
                hoverEnabled: true
                // Not checkable: `checked` is a one-way reflection of
                // `current`, never a user-toggled state. Qt derives no
                // accessible checked state for a non-checkable button, so the
                // state is exposed through `Accessible.checked` and `Accessible.selected`
                // below.
                checkable: false
                // Only the current segment is in the Tab order.
                focusPolicy: current ? Qt.TabFocus : Qt.NoFocus
                focus: current

                Accessible.role: Accessible.PageTab
                Accessible.name: modelData.label
                Accessible.checked: current
                Accessible.selected: current

                onClicked: control.activated(index)

                background: Rectangle {
                    id: segmentBackground
                    objectName: "segmentPlate"
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
                            objectName: "segmentFill"
                            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionFast
                        }
                    }
                }

                CelestinaFocusRing {
                    target: segment
                    cornerRadius: CelestinaTheme.radiusButton - CelestinaTheme.spaceXs
                    shown: segment.visualFocus
                }

                contentItem: Item {
                    implicitWidth: content.implicitWidth
                    implicitHeight: content.implicitHeight

                    Row {
                        id: content
                        anchors.centerIn: parent
                        spacing: CelestinaTheme.spaceSm

                        CelestinaIcon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: segment.modelData.icon
                            width: CelestinaTheme.iconMd
                            height: width
                            tone: segment.current ? CelestinaIcon.Primary : CelestinaIcon.Secondary
                        }

                        Text {
                            id: labelText
                            objectName: "segmentLabel"
                            anchors.verticalCenter: parent.verticalCenter
                            visible: !control.iconOnly
                            text: segment.modelData.label
                            textFormat: Text.PlainText
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontBody
                            font.weight: segment.current ? CelestinaTheme.weightDemiBold : CelestinaTheme.weightRegular
                            color: segment.current ? CelestinaTheme.text : CelestinaTheme.textMuted
                        }
                    }
                }
            }
        }
    }
}
