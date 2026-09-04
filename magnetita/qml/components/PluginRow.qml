import QtQuick
import org.celestina.magnetita 1.0

// A labelled toggle. The whole row is the target, not only the switch: a
// label beside a switch reads as part of the control, and a click on it that
// did nothing was the most common dead click on the settings page.
Item {
    id: root

    required property string label
    required property bool enabledFlag
    signal toggleRequested

    height: 46

    // Lit under the pointer so the row reads as one control — except while
    // the pointer is on the switch itself, whose own tint is the shared
    // control's business: two fills lighting at once read as two controls.
    //
    // Stacking dependency: `toggle` is declared after `rowArea` and so sits
    // above it. The switch consumes its own press, so `rowArea.pressed` is
    // only ever true for a press on the label side, while `containsMouse`
    // stays true across the whole row and is masked here by `toggle.hovered`.
    CelestinaRowHighlight {
        anchors.fill: parent
        hovered: rowArea.containsMouse && !toggle.hovered
        pressed: rowArea.pressed
        Accessible.ignored: true
    }

    // Behind the switch, so a press on the switch itself still belongs to
    // the switch — its keyboard and focus behaviour stay exactly as they are.
    MouseArea {
        id: rowArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggleRequested()
    }

    Text {
        anchors.left: parent.left
        anchors.leftMargin: 16
        anchors.right: toggle.left
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        text: root.label
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
        elide: Text.ElideRight
    }

    CelestinaSwitch {
        id: toggle
        anchors.right: parent.right
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        checked: root.enabledFlag
        Accessible.name: root.label

        // A click is only a request. Re-bind immediately so the switch keeps
        // showing the daemon's confirmed state instead of an optimistic one:
        // Magnetita reflects only snapshots `org.celestina.Devices1` confirms,
        // and a toggle that painted itself on before the daemon agreed would
        // be a second truth the local contract forbids.
        onClicked: {
            checked = Qt.binding(function() { return root.enabledFlag })
            root.toggleRequested()
        }
    }
}
