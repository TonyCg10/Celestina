pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

GlassContextMenu {
    id: root

    required property var rows
    required property int selectedIndex

    signal filterChosen(int index)

    Instantiator {
        model: root.rows

        delegate: GlassMenuItem {
            id: choice

            required property int index
            required property var modelData
            text: modelData.label
            choice: true
            current: choice.index === root.selectedIndex
            onTriggered: root.filterChosen(choice.index)
        }

        onObjectAdded: function(index, object) {
            root.insertItem(index, object)
        }
        onObjectRemoved: function(index, object) {
            root.removeItem(object)
        }
    }
}
