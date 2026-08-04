import QtQuick
import org.celestina.fluorita 1.0

// The context menu for one library item.
//
// The suite already owns this control: `GlassContextMenu` is a real `Menu` —
// focus, Escape, arrows, Enter, an input barrier so the dismissing click does
// not also land on the row underneath — wearing the shared glass. Siderita's
// menus are the same component. Rebuilding the anatomy here would have been a
// second menu language in one desktop, which is exactly what the shared style
// exists to prevent.
//
// It owns no truth about the item, only which one it was opened on.
GlassContextMenu {
    id: menu

    // The item the menu acts on, set by the host before it opens.
    property string targetPath: ""
    property string targetName: ""

    signal trashRequested(string path)
    signal propertiesRequested(string path)

    GlassMenuItem {
        text: qsTr("Mover a la papelera")
        icon.name: "user-trash"
        icon.source: CelestinaTheme.fallbackIcon("user-trash")
        Accessible.description: qsTr(
            "Mueve %1 a la papelera del escritorio, desde donde se puede restaurar").arg(menu.targetName)
        onTriggered: menu.trashRequested(menu.targetPath)
    }

    GlassMenuItem {
        text: qsTr("Propiedades")
        icon.name: "info"
        icon.source: CelestinaTheme.fallbackIcon("info")
        onTriggered: menu.propertiesRequested(menu.targetPath)
    }
}
