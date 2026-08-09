// How many notifications are waiting, and whether they are being held back.
//
// The bell is a permanent entry point to notification history. A count appears
// only when there is one to show, while the crossed bell carries quiet mode.
// Whether Celestina currently owns the notification server changes the content
// of the centre, not whether the person can reach that centre.
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

    implicitWidth: content.implicitWidth
    implicitHeight: 26
    visible: true

    Accessible.role: Accessible.Button
    Accessible.name: !root.serving
            ? qsTr("Notificaciones no disponibles")
            : root.quiet
            ? qsTr("Notificaciones silenciadas, %1 sin leer").arg(root.unread)
            : qsTr("%1 notificaciones sin leer").arg(root.unread)
    Accessible.onPressAction: root.historyRequested()

    Row {
        id: content

        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceXs

        CelestinaIcon {
            objectName: "celestina-notification-icon"
            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.iconSm
            height: CelestinaTheme.iconSm
            name: root.quiet ? "bell-off" : "bell"
            tone: CelestinaIcon.Primary
            Accessible.ignored: true
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.unread > 0
            text: root.unread
            // Silenced still counts: the number is what is waiting, not what
            // interrupted.
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontTitle
        }

    }

    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        onClicked: (mouse) => {
            if (mouse.button === Qt.RightButton) {
                if (root.serving)
                    root.quietToggled();
                return;
            }
            root.historyRequested();
        }
    }
}
