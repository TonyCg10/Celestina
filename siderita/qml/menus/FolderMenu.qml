import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// ─── FolderMenu ─────────────────────────────────────────────────────────────
// Menú contextual del espacio vacío de la carpeta: crear, pegar, deshacer,
// seleccionar todo, terminal, olvidar la vista, actualizar, ocultos. El
// controlador, el panel (para "seleccionar todo") y el diálogo de nombre (para
// crear) llegan por propiedad; "abrir en pestaña nueva" sale como señal. Así el
// componente no alcanza ningún id de fuera.
// ──────────────────────────────────────────────────────────────────────────────
GlassContextMenu {
    id: root

    property var controller
    property var panel        // mainPanel: "seleccionar todo"
    property var namePrompt   // diálogo de nombre: crear carpeta / archivo
    signal newTabRequested(string path, bool foreground)

    // Refresh paste availability so "Pegar" also lights up for file
    // URIs another manager placed on the system clipboard.
    onAboutToShow: root.controller.refreshPasteState()

    GlassMenuItem {
        text: "Nueva carpeta"
        icon.name: "folder-new"
        icon.source: CelestinaTheme.fallbackIcon("folder")
        onTriggered: root.namePrompt.openCreate("folder")
    }

    GlassMenuItem {
        text: "Nuevo archivo"
        icon.name: "document-new"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.namePrompt.openCreate("file")
    }

    GlassMenuItem {
        text: "Pegar"
        enabled: root.controller.canPaste
        icon.name: "edit-paste"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.controller.paste()
    }

    GlassMenuItem {
        text: root.controller.canUndo ? root.controller.undoLabel : "Deshacer"
        visible: root.controller.canUndo
        height: visible ? implicitHeight : 0
        icon.name: "edit-undo"
        icon.source: CelestinaTheme.fallbackIcon("view-refresh")
        onTriggered: root.controller.undo()
    }

    MenuSeparator {
        contentItem: Rectangle {
            implicitHeight: 1
            color: CelestinaTheme.divider
        }
    }

    GlassMenuItem {
        text: "Seleccionar todo"
        onTriggered: root.panel.selectAll()
    }

    GlassMenuItem {
        text: "Abrir en pestaña nueva"
        icon.name: "tab-new"
        icon.source: CelestinaTheme.fallbackIcon("folder")
        onTriggered: root.newTabRequested(root.controller.currentPathKey, true)
    }

    GlassMenuItem {
        text: "Abrir terminal aquí"
        icon.name: "utilities-terminal"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.controller.openTerminal()
    }

    // Only offered once this folder actually remembers something, and
    // it says plainly what it drops.
    GlassMenuItem {
        text: "Olvidar la vista de esta carpeta"
        visible: root.controller.folderViewPinned
        height: visible ? implicitHeight : 0
        icon.name: "edit-clear"
        icon.source: CelestinaTheme.fallbackIcon("view-refresh")
        onTriggered: root.controller.forgetFolderView()
    }

    GlassMenuItem {
        text: "Actualizar"
        icon.name: "view-refresh"
        icon.source: CelestinaTheme.fallbackIcon("view-refresh")
        onTriggered: root.controller.refresh()
    }

    GlassMenuItem {
        text: root.controller.showHidden
              ? "Ocultar elementos ocultos"
              : "Mostrar elementos ocultos"
        onTriggered: root.controller.toggleHidden()
    }
}
