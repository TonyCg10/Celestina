import QtQuick
import org.celestina.siderita 1.0

// ─── FolderSortMenu ─────────────────────────────────────────────────────────
// El menú del botón "Orden:" de la vista de carpeta. Elige el campo de orden
// (nombre, tamaño, fecha, tipo) sobre el controlador de la pestaña, que llega
// por propiedad — el componente no alcanza ningún id de fuera. `backdropSource`
// lo hereda de GlassContextMenu y lo fija quien lo instancia.
// ──────────────────────────────────────────────────────────────────────────────
GlassContextMenu {
    id: root

    // El controlador de la pestaña, inyectado por la vista de carpeta.
    property var controller

    GlassMenuItem {
        text: "Nombre"
        choice: true
        current: root.controller.sortField === 0
        onTriggered: root.controller.changeSortField(0)
    }

    GlassMenuItem {
        text: "Tamaño"
        choice: true
        current: root.controller.sortField === 1
        onTriggered: root.controller.changeSortField(1)
    }

    GlassMenuItem {
        text: "Fecha de modificación"
        choice: true
        current: root.controller.sortField === 2
        onTriggered: root.controller.changeSortField(2)
    }

    GlassMenuItem {
        text: "Tipo"
        choice: true
        current: root.controller.sortField === 3
        onTriggered: root.controller.changeSortField(3)
    }
}
