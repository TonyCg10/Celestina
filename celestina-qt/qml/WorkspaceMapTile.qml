// One window: its application's mark, what it is, and what it is called.
//
// There is no preview here and there cannot be one. Wayland gives a client no
// access to another client's buffers, and — the part that settles it — a
// workspace nobody is looking at is not being drawn at all, so even capturing
// the whole output would find nothing to crop.
//
// A full-width row rather than a tile in a grid, because the column layout it
// replaced made every name too narrow to read: a map you cannot read the labels
// of has stopped answering the question it exists for. The arrangement is still
// told, by the order and by the column each row belongs to; what is gone is the
// squeezing.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Item {
    id: root

    // One window's published row: id, title, appId, focused, floating, urgent.
    required property var window
    required property BackdropInk ink
    property bool current: false
    signal activated()

    readonly property string windowId: window.id !== undefined ? window.id : ""
    readonly property string title: window.title !== undefined
                                    && window.title.length > 0
                                    ? window.title : qsTr("Sin título")
    // The application's own id. It is not a marketing name — resolving one would
    // mean reading desktop entries, which is a lookup this shell does not do —
    // but it is what the compositor knows the program as, and it tells two
    // windows of different applications apart when their titles do not.
    readonly property string appId: window.appId !== undefined ? window.appId : ""
    readonly property bool focused: window.focused === true
    readonly property bool urgent: window.urgent === true

    implicitHeight: CelestinaTheme.rowHeight - CelestinaTheme.spaceMd
    Accessible.role: Accessible.Button
    Accessible.name: root.appId.length > 0
                     ? qsTr("%1 — %2").arg(root.appId).arg(root.title)
                     : root.title
    Accessible.description: qsTr("Va a esta ventana")
    Accessible.onPressAction: root.activated()

    Rectangle {
        anchors.fill: parent
        // Rounded square rather than a capsule: it is the shape the rest of the
        // suite gives a row, and a row is what this is.
        radius: CelestinaTheme.radiusSm
        color: {
            if (pointer.pressed)
                return root.ink.selectedFill;

            if (pointer.containsMouse || root.current)
                return root.ink.hoverFill;

            return CelestinaTheme.clear;
        }
        border.width: root.urgent || root.current
                      ? CelestinaTheme.borderHairline : 0
        border.color: root.urgent ? root.ink.danger : root.ink.focus

        Behavior on color {
            enabled: !CelestinaTheme.reducedMotion

            ColorAnimation {
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }

        }

    }

    Row {
        anchors.fill: parent
        anchors.leftMargin: CelestinaTheme.spaceSm
        anchors.rightMargin: CelestinaTheme.spaceSm
        spacing: CelestinaTheme.spaceSm

        Rectangle {
            id: mark

            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.glyphTile - CelestinaTheme.spaceXs
            height: width
            // A rounded square, matching the row and the suite's own glyph
            // tiles, rather than the circle this used to draw.
            radius: CelestinaTheme.radiusSm
            color: root.urgent
                   ? CelestinaTheme.withAlpha(
                         root.ink.danger,
                         CelestinaTheme.decorationOpacitySoft / 3)
                   : (root.focused ? root.ink.accentFill
                                   : root.ink.controlFill)

            Image {
                id: icon

                anchors.centerIn: parent
                // Another application's own icon, looked up by the id it
                // reports. An id no installed theme knows resolves to nothing,
                // which is ordinary rather than an error: the initial below
                // stands in, and the two lines beside it still say what this is.
                source: root.appId.length > 0
                        ? "image://appicon/" + encodeURIComponent(root.appId) : ""
                sourceSize.width: CelestinaTheme.iconSm
                                  * Screen.devicePixelRatio
                sourceSize.height: CelestinaTheme.iconSm
                                   * Screen.devicePixelRatio
                width: CelestinaTheme.iconSm
                height: CelestinaTheme.iconSm
                fillMode: Image.PreserveAspectFit
                visible: status === Image.Ready
            }

            Text {
                anchors.centerIn: parent
                visible: !icon.visible
                text: (root.appId.length > 0 ? root.appId : root.title)
                      .charAt(0).toUpperCase()
                textFormat: Text.PlainText
                color: root.ink.muted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                font.weight: CelestinaTheme.weightDemiBold
            }

        }

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(0, parent.width - mark.width - parent.spacing)
            spacing: 0

            Text {
                width: parent.width
                text: root.title
                // A window's title is whatever its client set it to, so it is
                // shown as characters rather than guessed at as markup.
                textFormat: Text.PlainText
                color: root.ink.primary
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                font.weight: root.focused ? CelestinaTheme.weightDemiBold
                                          : CelestinaTheme.weightRegular
                elide: Text.ElideRight
                maximumLineCount: 1
            }

            Text {
                width: parent.width
                visible: root.appId.length > 0
                text: root.appId
                textFormat: Text.PlainText
                color: root.ink.faint
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
                elide: Text.ElideRight
                maximumLineCount: 1
            }

        }

    }

    MouseArea {
        id: pointer

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }

}
