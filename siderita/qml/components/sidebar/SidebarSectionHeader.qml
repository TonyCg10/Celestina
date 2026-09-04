import QtQuick
import org.celestina.siderita 1.0

// Canonical sidebar section header. Natural headers toggle their section;
// sticky copies use the same component but route activation back to the
// section's original position.
Item {
    id: root

    required property string title
    property real textScale: 1.0
    property real iconScale: 1.0
    property bool collapsible: true
    property bool collapsed: false
    property bool sticky: false
    property bool interactive: true
    property string trailingText: ""

    signal activated
    signal trailingActivated

    implicitHeight: CelestinaTheme.controlHeightXs
    height: implicitHeight
    Accessible.role: interactive ? Accessible.Button : Accessible.StaticText
    Accessible.name: title
    Accessible.onPressAction: if (root.interactive) root.activated()

    Rectangle {
        anchors.fill: parent
        visible: root.sticky
        color: CelestinaTheme.card
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: CelestinaTheme.borderHairline
        visible: root.sticky
        color: CelestinaTheme.divider
    }

    // The header is a row like any other: it lights under the pointer and
    // darkens under the press, but not while the pointer is on the trailing
    // action, which paints its own state.
    CelestinaRowHighlight {
        anchors.fill: parent
        anchors.leftMargin: CelestinaTheme.spaceXs / 2
        anchors.rightMargin: CelestinaTheme.spaceXs / 2
        visible: root.interactive
        hovered: headerMouse.containsMouse && !trailingMouse.containsMouse
        pressed: headerMouse.pressed
    }

    SidebarChevron {
        id: chevron
        x: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        iconScale: root.iconScale
        collapsed: root.collapsed
        visible: root.collapsible
    }

    CelestinaSectionLabel {
        id: label
        x: chevron.x + chevron.width
           + CelestinaTheme.spaceSm + CelestinaTheme.spaceXs / 2
        anchors.verticalCenter: parent.verticalCenter
        width: trailingLabel.visible
               ? trailingIcon.x - x - CelestinaTheme.spaceSm
               : parent.width - x - CelestinaTheme.spaceMd
        text: root.title
        textScale: root.textScale
        elide: Text.ElideRight
    }

    MouseArea {
        id: headerMouse
        anchors.fill: parent
        enabled: root.interactive
        hoverEnabled: true
        cursorShape: root.interactive ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.activated()
    }

    // The count of hidden entries, glyph first: the eye says what the number
    // brings back.
    CelestinaIcon {
        id: trailingIcon
        z: 2
        anchors.right: trailingLabel.left
        anchors.rightMargin: CelestinaTheme.spaceXs
        anchors.verticalCenter: parent.verticalCenter
        visible: trailingLabel.visible
        width: Math.round(CelestinaTheme.iconSm * root.iconScale)
        height: width
        name: "eye"
        fallbackName: "eye"
        tone: trailingMouse.containsMouse ? CelestinaIcon.Accent
                                          : CelestinaIcon.Secondary
    }

    Text {
        id: trailingLabel
        z: 2
        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        visible: root.trailingText.length > 0
        text: root.trailingText
        color: trailingMouse.containsMouse ? CelestinaTheme.accent
                                           : CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: Math.round(CelestinaTheme.fontMini * root.textScale)

        MouseArea {
            id: trailingMouse
            anchors.fill: parent
            // The text is a caption; the target is the header's full 30 px,
            // and it reaches back over the eye glyph.
            anchors.leftMargin: -(CelestinaTheme.spaceXs * 2 + trailingIcon.width)
            anchors.rightMargin: -CelestinaTheme.spaceXs
            anchors.topMargin: -Math.max(0, Math.round(
                (CelestinaTheme.controlHeightXs - parent.height) / 2))
            anchors.bottomMargin: -Math.max(0, Math.round(
                (CelestinaTheme.controlHeightXs - parent.height) / 2))
            enabled: root.interactive
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.trailingActivated()
        }
    }
}
