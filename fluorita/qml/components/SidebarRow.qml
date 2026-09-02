import QtQuick
import org.celestina.fluorita 1.0

// One line of the library sidebar.
//
// A rounded highlight, not a card. The suite's sidebar rows carry their state
// in the fill — transparent at rest, `surfaceHover` under the pointer,
// `badgeAccentFill` when they are the current one — because a border around
// every line turns a short list into a stack of boxes and hides which one is
// selected. Siderita's bookmark rows are the same shape; this is that anatomy
// for a list whose entries are folders instead of places.
Item {
    id: row

    required property string label
    property string description: ""
    property string iconName: "folder"
    property bool current: false
    // For the row that is an action rather than a destination.
    property bool muted: false

    signal activated
    // Emitted in the row's own coordinates. The host decides where a menu goes.
    signal menuRequested(real x, real y)

    Accessible.role: Accessible.ListItem
    Accessible.name: row.label
    Accessible.description: row.description
    Accessible.focusable: true
    Accessible.onPressAction: row.activated()

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 2
        anchors.rightMargin: 2
        radius: CelestinaTheme.radiusSm
        color: !row.enabled
               ? CelestinaTheme.clear
               : row.current
                 ? CelestinaTheme.badgeAccentFill
                 : pointer.containsMouse
                   ? CelestinaTheme.surfaceHover
                   : CelestinaTheme.clear

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
            }
        }
    }

    CelestinaIcon {
        id: glyph

        x: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        width: CelestinaTheme.iconSm
        height: width
        sourceSize: Qt.size(width, height)
        name: row.iconName
        fallbackName: "folder"
        tone: row.current ? CelestinaIcon.Accent
                          : row.muted ? CelestinaIcon.Secondary
                                      : CelestinaIcon.Primary
        opacity: row.enabled ? 1 : CelestinaTheme.disabledContentOpacity
    }

    Text {
        anchors.left: glyph.right
        anchors.leftMargin: CelestinaTheme.spaceSm
        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        text: row.label
        color: row.current
               ? CelestinaTheme.accent
               : row.muted ? CelestinaTheme.textMuted : CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
        font.weight: row.current ? CelestinaTheme.weightMedium
                                 : CelestinaTheme.weightRegular
        elide: Text.ElideRight
        opacity: row.enabled ? 1 : CelestinaTheme.disabledContentOpacity
    }

    MouseArea {
        id: pointer

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: function(mouse) {
            if (mouse.button === Qt.RightButton) {
                row.menuRequested(mouse.x, mouse.y);
                return;
            }
            row.activated();
        }
    }

    // A row that stands on its own in the tab chain — the add-folder row — is
    // activated from the keyboard the way a list activates its current row.
    // Inside a list the view holds the focus and these never fire.
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter
                || event.key === Qt.Key_Space) {
            row.activated();
            event.accepted = true;
        }
    }

    // The pointer never gives a row focus, so the ring only ever answers the
    // keyboard.
    CelestinaFocusRing {
        target: row
        cornerRadius: CelestinaTheme.radiusSm
        shown: row.activeFocus
    }
}
