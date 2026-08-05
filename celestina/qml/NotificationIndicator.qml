// How many notifications are waiting, and whether they are being held back.
//
// The panel's flank reserved this place for R4. It shows a number only when
// there is one to show: a permanent badge reading zero is furniture, not
// information. Do-not-disturb is the exception — that state is always visible,
// because a person who silenced their session needs to be able to tell.
//
// It reads as text rather than a glyph for the same reason `AudioLevel` does:
// the suite's icon catalogue is closed and vendored, it has no bell, and
// inventing one would put non-canonical artwork into a set that is canonical
// everywhere else.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: root

    // The `notifications` provider's fields, or `undefined` when this shell is
    // not the session's notification server.
    required property var reading

    signal historyRequested()
    signal quietToggled()

    readonly property bool serving: reading !== undefined
                                    && reading.unread !== undefined
    readonly property int unread: root.serving ? root.reading.unread : 0
    readonly property bool quiet: root.serving && root.reading.quiet === true
    // Nothing to say is nothing to show: this shell does not keep a widget on
    // the panel to report that nothing happened.
    readonly property bool worthShowing: root.serving
                                         && (root.unread > 0 || root.quiet)

    implicitWidth: worthShowing ? content.implicitWidth : 0
    implicitHeight: 26
    visible: worthShowing

    Accessible.role: Accessible.Button
    Accessible.name: root.quiet
            ? qsTr("Notificaciones silenciadas, %1 sin leer").arg(root.unread)
            : qsTr("%1 notificaciones sin leer").arg(root.unread)
    Accessible.onPressAction: root.historyRequested()

    Row {
        id: content

        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceXs

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.unread > 0
            text: root.unread
            // Silenced still counts: the number is what is waiting, not what
            // interrupted.
            color: root.quiet ? CelestinaTheme.textMuted : CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontCaption
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            // The word appears only while the session is silenced: the rest of
            // the time the number needs no caption.
            visible: root.quiet
            text: qsTr("silenciadas")
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }
    }

    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        onClicked: (mouse) => {
            if (mouse.button === Qt.RightButton) {
                root.quietToggled();
                return;
            }
            root.historyRequested();
        }
    }
}
