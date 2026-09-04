pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.magnetita 1.0

// One mirror setting, as the closed set of values it actually accepts.
//
// A row of buttons rather than a dropdown: there are never more than four
// choices, they are all short, and seeing the alternatives beside the current
// one is the whole point — "how sharp is this" is answered by what else was on
// offer. The selected value is the daemon's confirmed one, so a press that the
// daemon refuses simply leaves the row where it was.
Item {
    id: root

    required property string label
    required property var options
    required property var labels
    required property string current
    signal chosen(string value)

    implicitHeight: Math.max(caption.implicitHeight, choices.implicitHeight)

    Text {
        id: caption
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(parent.width * 0.4, 170)
        text: root.label
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
        elide: Text.ElideRight
    }

    RowLayout {
        id: choices
        anchors.left: caption.right
        anchors.leftMargin: CelestinaTheme.spaceSm
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceXs

        Repeater {
            model: root.options.length

            // Text, not glyphs: the labels are the values themselves ("1080",
            // "60 fps", "4 Mb/s") and no icon would carry them.
            delegate: CelestinaButton {
                required property int index

                Layout.fillWidth: true
                text: root.labels[index]
                role: CelestinaButton.Ghost
                // Radio-like among peers: the checked one wears Selected.
                checkable: true
                checked: root.options[index] === root.current
                // A checkable Button flips `checked` on click; re-bind so the
                // row only ever shows the daemon's confirmed value.
                onClicked: {
                    checked = Qt.binding(function() {
                        return root.options[index] === root.current
                    })
                    root.chosen(root.options[index])
                }
            }
        }
    }
}
