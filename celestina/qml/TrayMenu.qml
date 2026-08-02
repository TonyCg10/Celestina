// Another application's menu, in this panel's surface.
//
// The entries are that application's, read over `com.canonical.dbusmenu` and
// already bounded by the host; this only draws them and reports which one was
// chosen. What choosing does is the application's business — the panel learns
// nothing back, which is why nothing here waits for a result.
//
// Submenus are drawn indented in place rather than as menus that open sideways.
// The host flattens the tree it was given, and a sideways menu is a second
// surface to place, dismiss and return focus from: that deserves its own
// decision rather than one made in passing.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: menuWindow

    // The entries the host read, already stripped, bounded and flattened.
    // `var` is necessary: QML has no typed map-list.
    required property var entries
    required property bool reducedMotion
    signal chosen(int entryId)
    signal dismissed()

    // The card is inset from the surface by the room its shadow needs, exactly
    // as the panel's own menu is.
    readonly property int shadowMargin: CelestinaTheme.shadowBlur
                                        + CelestinaTheme.shadowOffsetY

    width: menu.width + shadowMargin * 2
    height: menu.implicitHeight + shadowMargin * 2
    color: CelestinaTheme.clear
    title: qsTr("Menú de la bandeja")

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = reducedMotion;
        menu.open();
    }

    Item {
        id: scene

        anchors.fill: parent

        GlassContextMenu {
            id: menu

            backdropSource: scene
            x: menuWindow.shadowMargin
            y: menuWindow.shadowMargin
            // This surface exists only to carry this menu, so the shared margin
            // has nothing to keep it clear of.
            margins: -1
            onClosed: menuWindow.dismissed()

            Instantiator {
                model: menuWindow.entries
                onObjectAdded: (index, object) => menu.insertItem(index, object)
                onObjectRemoved: (index, object) => menu.removeItem(object)

                delegate: GlassMenuItem {
                    required property var modelData

                    text: modelData.separator ? "———" : modelData.label
                    enabled: modelData.enabled
                    // Nesting arrives flattened, so depth is shown as depth.
                    leftPadding: CelestinaTheme.spaceMd
                                 + modelData.depth * CelestinaTheme.spaceMd
                    // A toggle the application owns: the panel shows its state
                    // and never predicts the next one.
                    choice: modelData.toggleType.length > 0
                    current: modelData.toggleState === 1
                    onTriggered: menuWindow.chosen(modelData.id)
                }

            }

        }

    }

}
