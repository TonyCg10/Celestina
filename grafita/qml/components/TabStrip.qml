import QtQuick
import org.celestina.grafita 1.0

// The open documents, one row.
//
// Always there, including with a single document: hiding it meant the strip
// appeared and shifted the whole editor down the moment a second file arrived,
// and it left the "new tab" button nowhere to be found until you already had
// two. A steady row is worth the few pixels.
Item {
    id: root

    required property var tabs          // ListModel of open documents
    required property int current
    // Reads a tab's live state from the window, which owns the sessions.
    required property var titleFor      // function(index) -> string
    required property var dirtyFor      // function(index) -> bool

    signal selected(int index)
    signal closeRequested(int index)
    signal newRequested

    // A revision the window bumps so the delegates re-read titles that live
    // outside the model — a document's name arrives after its file opens.
    property int revision: 0

    implicitHeight: strip.height + CelestinaTheme.spaceSm

    Rectangle {
        anchors.fill: parent
        color: CelestinaTheme.surface
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.divider
    }

    Row {
        id: strip
        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceXs

        Repeater {
            model: root.tabs

            delegate: Rectangle {
                id: tab
                required property int index

                readonly property bool active: index === root.current
                // `revision` is read so the binding re-evaluates when a
                // document finishes opening and finally has a name.
                readonly property string label: {
                    root.revision
                    return root.titleFor(index)
                }
                readonly property bool dirty: {
                    root.revision
                    return root.dirtyFor(index)
                }

                width: title.implicitWidth + closeButton.width + CelestinaTheme.space2xl
                height: CelestinaTheme.controlHeight
                radius: CelestinaTheme.radiusSm
                // An idle tab shows the strip behind it rather than a colour of
                // its own; `withAlpha` at zero is the theme's way of saying that
                // without writing a literal.
                color: tab.active ? CelestinaTheme.surfaceSelected
                                  : (hover.hovered
                                     ? CelestinaTheme.surfaceHover
                                     : CelestinaTheme.withAlpha(CelestinaTheme.surface, 0))

                Accessible.role: Accessible.PageTab
                Accessible.name: tab.dirty ? tab.label + ", sin guardar" : tab.label
                Accessible.selected: tab.active

                HoverHandler { id: hover }

                MouseArea {
                    anchors.fill: parent
                    // Middle click closes, the way every tabbed thing does.
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                    onClicked: function(mouse) {
                        if (mouse.button === Qt.MiddleButton)
                            root.closeRequested(tab.index)
                        else
                            root.selected(tab.index)
                    }
                }

                Text {
                    id: title
                    anchors.left: parent.left
                    anchors.leftMargin: CelestinaTheme.spaceSm
                    anchors.verticalCenter: parent.verticalCenter
                    text: tab.dirty ? tab.label + " •" : tab.label
                    elide: Text.ElideMiddle
                    color: tab.active ? CelestinaTheme.text : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                }

                CelestinaIconButton {
                    id: closeButton
                    anchors.right: parent.right
                    anchors.rightMargin: CelestinaTheme.spaceXs
                    anchors.verticalCenter: parent.verticalCenter
                    iconName: "x"
                    Accessible.role: Accessible.Button
                    Accessible.name: "Cerrar " + tab.label
                    onClicked: root.closeRequested(tab.index)
                }
            }
        }

        CelestinaIconButton {
            anchors.verticalCenter: parent.verticalCenter
            iconName: "plus"
            Accessible.role: Accessible.Button
            Accessible.name: "Pestaña nueva"
            onClicked: root.newRequested()
        }
    }
}
