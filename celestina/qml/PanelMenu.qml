// The panel's context menu, hosted in its own compositor surface.
//
// `GlassContextMenu` renders inside its window's scene by design, and the
// panel's scene is 40 px tall — so the menu needs a surface of its own. This
// file is only the *content* of that surface; the host maps it as the layer
// surface R0-E chose.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: menuWindow

    // The output's workspaces as the panel had them when the menu opened. A
    // menu is a momentary question; it does not follow the compositor while it
    // is up.
    required property var workspaces
    required property bool reducedMotion
    // The card is inset from the surface by exactly the room its drop shadow
    // needs — `GlassSurface` draws the shadow outside the card and the window
    // would otherwise clip it. The host offsets the anchor by this much so the
    // card, not the surface, lands where the click was.
    readonly property int shadowMargin: CelestinaTheme.shadowBlur
                                        + CelestinaTheme.shadowOffsetY
    // Every item is a real request, routed exactly like a click on the strip.
    signal activated(string output, int index)
    signal dismissed()

    // Height comes from what the menu's content *implies*, never from the
    // menu's own laid-out height: `Popup` fits itself to its window, so a
    // window that sized itself to the laid-out popup shrank both of them by one
    // margin per pass until the surface was a sliver. Width comes from the
    // menu, because the shared component fixes it to a token rather than
    // deriving it from anything here — its `implicitWidth` counts only padding.
    width: menu.width + shadowMargin * 2
    height: menu.implicitHeight + shadowMargin * 2
    color: CelestinaTheme.clear
    title: qsTr("Menú del panel")

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = reducedMotion;
        menu.open();
    }

    Item {
        id: scene

        anchors.fill: parent

        GlassContextMenu {
            id: menu

            // Nothing of the shell is behind this surface — the compositor's
            // own backdrop is. The glass therefore reads as its tint here; a
            // surface-level blur belongs to a later phase.
            backdropSource: scene
            x: menuWindow.shadowMargin
            y: menuWindow.shadowMargin
            // The shared margin keeps a menu clear of the edges of the window
            // it pops up in. This surface exists only to carry this menu, so
            // that clamp has nothing to protect and only fights the size above.
            margins: -1
            onClosed: menuWindow.dismissed()

            Instantiator {
                id: items

                model: menuWindow.workspaces
                onObjectAdded: (index, object) => menu.insertItem(index, object)
                onObjectRemoved: (index, object) => menu.removeItem(object)

                delegate: GlassMenuItem {
                    required property var modelData

                    text: qsTr("Ir al espacio %1").arg(modelData.label)
                    choice: true
                    current: modelData.active
                    onTriggered: menuWindow.activated(modelData.output, modelData.index)
                }

            }

        }

    }

}
