import QtQuick
import org.celestina.siderita 1.0

// ─── PathMenu ───────────────────────────────────────────────────────────────
// Menú contextual de la barra de ruta / migas de pan: actúa sobre la ruta
// actual (marcar, abrir en pestaña nueva). El controlador llega por propiedad;
// "abrir en pestaña nueva" sale como señal para que la vista de carpeta la
// reenvíe a su propio `requestNewTab` — el componente no alcanza ids de fuera.
// ──────────────────────────────────────────────────────────────────────────────
GlassContextMenu {
    id: root

    property var controller
    signal newTabRequested(string path, bool foreground)

    GlassMenuItem {
        text: "Añadir a marcadores"
        icon.name: "bookmark-new"
        icon.source: CelestinaTheme.fallbackIcon("folder")
        onTriggered: root.controller.addBookmark(root.controller.currentPath)
    }

    GlassMenuItem {
        text: "Abrir en pestaña nueva"
        icon.name: "tab-new"
        icon.source: CelestinaTheme.fallbackIcon("folder")
        onTriggered: root.newTabRequested(root.controller.currentPath, true)
    }
}
