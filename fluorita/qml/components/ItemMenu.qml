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

    // The item the menu acts on, set by the host before it opens: its path
    // key, which is what the library's verbs take, and its name, which is only
    // ever read out.
    property string targetKey: ""
    property string targetName: ""
    // What this item admits, answered by the objects that own those matrices
    // before the menu is popped. An entry that would refuse is not shown: a
    // menu item that does nothing when clicked is the worst of both worlds,
    // because it looks like the application broke rather than like the thing
    // it never did.
    property bool editable: false
    property bool describable: false

    signal trashRequested(string key)
    signal propertiesRequested(string key)
    signal editRequested(string key)
    signal metadataRequested(string key)

    GlassMenuItem {
        visible: menu.editable
        text: qsTr("Editar")
        icon.name: "pencil"
        icon.source: CelestinaTheme.fallbackIcon("pencil")
        Accessible.description: qsTr("Abre %1 para girarla, recortarla o anotarla").arg(menu.targetName)
        onTriggered: menu.editRequested(menu.targetKey)
    }

    GlassMenuItem {
        text: qsTr("Mover a la papelera")
        icon.name: "user-trash"
        icon.source: CelestinaTheme.fallbackIcon("user-trash")
        Accessible.description: qsTr(
            "Mueve %1 a la papelera del escritorio, desde donde se puede restaurar").arg(menu.targetName)
        onTriggered: menu.trashRequested(menu.targetKey)
    }

    GlassMenuItem {
        visible: menu.describable
        text: qsTr("Datos del archivo")
        icon.name: "info"
        icon.source: CelestinaTheme.fallbackIcon("info")
        Accessible.description: qsTr(
            "Muestra lo que %1 dice de sí mismo y permite corregirlo o quitarlo").arg(menu.targetName)
        onTriggered: menu.metadataRequested(menu.targetKey)
    }

    GlassMenuItem {
        text: qsTr("Propiedades")
        icon.name: "info"
        icon.source: CelestinaTheme.fallbackIcon("info")
        onTriggered: menu.propertiesRequested(menu.targetKey)
    }
}
