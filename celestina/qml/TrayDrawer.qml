// The system tray: other applications' controls, kept out of the way.
//
// It is a drawer because the lived bar has one — a handful of icons that are
// almost never acted on should not spend the day occupying the panel. Two
// things are always visible regardless: an item asking for attention, which is
// the whole point of that status, and the toggle itself when there is anything
// to open.
//
// An item whose icon resolved to nothing shows its name instead of an empty
// slot. That is not rare: on this session one application names an icon no
// installed theme has and publishes no pixels either.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Controls

Row {
    id: root

    // The tray host's items. `var` is necessary: QML has no typed map-list.
    required property var items
    property bool open: false
    signal activated(string service, string path, int globalX, int globalY)
    signal secondaryActivated(string service, string path, int globalX, int globalY)
    // A right-click asks the host for this item's own menu.
    signal menuRequested(string service, string path, int globalX, int globalY)

    readonly property var attention: {
        const urgent = [];
        for (let index = 0; index < items.length; ++index) {
            if (items[index].status === "attention")
                urgent.push(items[index]);

        }
        return urgent;
    }
    readonly property var shown: open ? items : attention

    spacing: CelestinaTheme.spaceSm
    visible: items.length > 0

    Repeater {
        model: root.shown

        delegate: Item {
            id: entry

            required property var modelData
            readonly property bool hasIcon: modelData.iconSource !== undefined
                                            && modelData.iconSource.length > 0

            width: hasIcon ? 18 : nameLabel.implicitWidth
            height: 18
            anchors.verticalCenter: parent.verticalCenter
            Accessible.role: Accessible.Button
            Accessible.name: modelData.title
            Accessible.description: modelData.status === "attention"
                                    ? qsTr("Requiere atención") : ""
            Accessible.onPressAction: root.activated(modelData.service, modelData.path, 0, 0)

            Image {
                anchors.fill: parent
                visible: entry.hasIcon
                source: entry.hasIcon ? entry.modelData.iconSource : ""
                // The host already resolved this to the size it is drawn at;
                // asking for the same size keeps it from being resampled twice.
                sourceSize.width: 18
                sourceSize.height: 18
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                smooth: true
            }

            Text {
                id: nameLabel

                anchors.verticalCenter: parent.verticalCenter
                visible: !entry.hasIcon
                // An application whose icon nothing can resolve is still one
                // the user should be able to reach.
                text: entry.modelData.title
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                elide: Text.ElideRight
                width: Math.min(implicitWidth, 90)
            }

            Rectangle {
                anchors.top: parent.top
                anchors.right: parent.right
                width: 5
                height: 5
                radius: CelestinaTheme.radiusPill
                visible: entry.modelData.status === "attention"
                color: CelestinaTheme.danger
            }

            MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
                cursorShape: Qt.PointingHandCursor
                onClicked: (mouse) => {
                    const at = entry.mapToGlobal(0, entry.height);
                    if (mouse.button === Qt.RightButton) {
                        root.menuRequested(entry.modelData.service, entry.modelData.path, at.x, at.y);
                        return;
                    }
                    if (mouse.button === Qt.MiddleButton) {
                        root.secondaryActivated(entry.modelData.service, entry.modelData.path, at.x, at.y);
                        return;
                    }
                    root.activated(entry.modelData.service, entry.modelData.path, at.x, at.y);
                }
            }

        }

    }

    CelestinaIconButton {
        anchors.verticalCenter: parent.verticalCenter
        iconName: root.open ? "chevron-right" : "chevron-down"
        role: CelestinaButton.Ghost
        // `helpText` still names the button for AT-SPI (`CelestinaIconButton`
        // ties `Accessible.name` to it); the visible tooltip it also drives is
        // switched off here — a hover popup over a 40 px panel was landing on
        // top of the tray icons right next to it and swallowing their clicks.
        helpText: root.open ? qsTr("Ocultar la bandeja")
                            : qsTr("Mostrar la bandeja (%1)").arg(root.items.length)
        ToolTip.visible: false
        onClicked: root.open = !root.open
    }

}
