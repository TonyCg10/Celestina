// A monitor's workspaces, folded into one control because that monitor is not
// the one holding the focus.
//
// This exists for a state Niri creates and cannot describe: switch two monitors
// off and every workspace they held moves to the survivor, so a session
// configured with five per monitor draws fifteen equal pills in a row at exactly
// the moment there is least screen to read them on. The capsule is the other
// two monitors, named and counted, instead of ten pills that look like this
// monitor's.
//
// It hides workspaces, so it must never hide a reason to go to one: the count is
// always shown, and urgency inside is shown on the capsule itself.
//
// Clicking it is a focus request like any other. The capsule does not open its
// own group — the group opens because the focus arrived — so expansion has one
// rule rather than two.
import CelestinaStyle
import QtQuick

Item {
    id: capsule

    // The monitor this group belongs to; also what the capsule is labelled with.
    required property string outputName
    // How many workspaces are behind it.
    required property int count
    // Whether anything inside is asking for attention.
    required property bool urgent
    // A left click opens this group *in the strip*: a capsule is a container,
    // not a destination, so the gesture that would focus a workspace instead
    // shows the workspaces to choose from. Expansion is therefore something a
    // person does rather than something that follows the focus.
    signal expandRequested()
    // The right button asks for the map of everything this capsule folded.
    signal mapRequested(int globalX, int globalY)

    implicitWidth: Math.max(34, capsuleRow.implicitWidth + 14)
    implicitHeight: 26
    Accessible.role: Accessible.Button
    Accessible.name: urgent ? qsTr("%1, %2 espacios, requiere atención").arg(outputName).arg(count) : qsTr("%1, %2 espacios").arg(outputName).arg(count)
    Accessible.description: qsTr("Muestra los espacios de este monitor")
    // The same action the pointer's primary button takes, so assistive
    // technology is not offered a second, different meaning for one control.
    Accessible.onPressAction: capsule.expandRequested()

    Rectangle {
        anchors.fill: parent
        radius: CelestinaTheme.radiusPill
        color: pressArea.containsMouse ? CelestinaTheme.surfaceHover : CelestinaTheme.clear
        border.width: CelestinaTheme.borderHairline
        border.color: capsule.urgent ? CelestinaTheme.dangerBorder : CelestinaTheme.accentSoftBorder
    }

    Row {
        id: capsuleRow

        anchors.centerIn: parent
        spacing: CelestinaTheme.spaceXs

        Text {
            anchors.verticalCenter: parent.verticalCenter
            // An output name comes from the compositor, which took it from the
            // monitor's own EDID, so it is shown as characters.
            text: capsule.outputName
            textFormat: Text.PlainText
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: capsule.count
            color: CelestinaTheme.textMuted
            opacity: CelestinaTheme.mutedContentOpacity
            font.family: CelestinaTheme.sansFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontCaption
        }

    }

    Rectangle {
        anchors.top: parent.top
        anchors.right: parent.right
        width: 5
        height: 5
        radius: CelestinaTheme.radiusPill
        visible: capsule.urgent
        color: CelestinaTheme.danger
    }

    MouseArea {
        id: pressArea

        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        onClicked: (mouse) => {
            if (mouse.button === Qt.RightButton) {
                const anchor = capsule.mapToGlobal(0, capsule.height);
                capsule.mapRequested(anchor.x, anchor.y);
                return;
            }
            capsule.expandRequested();
        }
    }

}
