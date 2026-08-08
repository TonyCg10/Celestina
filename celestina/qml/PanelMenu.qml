// The panel's context menu, hosted in its own compositor surface.
//
// `GlassContextMenu` renders inside its window's scene by design, and the
// panel's scene is 40 px tall — so the menu needs a surface of its own. This
// file is only the *content* of that surface; `AnchoredMenu` owns the placement
// contract the host writes to and the dismissal the surface depends on.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

AnchoredMenu {
    id: root

    // The output's workspaces as the panel had them when the menu opened. A
    // menu is a momentary question; it does not follow the compositor while it
    // is up.
    required property var workspaces
    // Every item is a real request, routed exactly like a click on the strip.
    signal activated(string output, int index)

    title: qsTr("Menú del panel")

    Instantiator {
        model: root.workspaces
        onObjectAdded: (index, object) => root.menu.insertItem(index, object)
        onObjectRemoved: (index, object) => root.menu.removeItem(object)

        delegate: GlassMenuItem {
            required property var modelData

            text: qsTr("Ir al espacio %1").arg(modelData.label)
            choice: true
            current: modelData.active
            onTriggered: root.activated(modelData.output, modelData.index)
        }

    }

}
