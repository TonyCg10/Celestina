import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// ─── EntryContextMenu ───────────────────────────────────────────────────────
// Menú contextual de una entrada (fila o celda). El delegado que lo abre fija
// las propiedades `target*` sobre esta instancia y llama `popup`. El controlador,
// el panel (ayudas de selección y medios) y los tres diálogos (renombrar,
// renombrar en lote, cambiar icono) llegan por propiedad; "abrir en pestaña
// nueva" sale como señal. Así el componente no alcanza ningún id de fuera.
// ──────────────────────────────────────────────────────────────────────────────
GlassContextMenu {
    id: root

    // Fijadas por el delegado que abre el menú.
    property string targetToken: ""
    property string targetName: ""
    property bool targetDirectory: false
    property string targetPath: ""

    property var controller
    property var panel        // mainPanel: selección y medios
    property var namePrompt   // diálogo de nombre (renombrar uno)
    property var batchRename  // diálogo de renombrado en lote
    property var iconPicker   // diálogo de cambiar icono
    signal newTabRequested(string path, bool foreground)

    // How many entries the batch-capable verbs (copy/cut/trash) will act
    // on: the whole selection when the right-clicked entry is part of a
    // multi-selection, otherwise just this one.
    readonly property int actingCount: root.panel.actingCount(root.targetToken)
    readonly property bool multi: root.actingCount > 1

    // ── Trash-only actions ──
    GlassMenuItem {
        text: "Restaurar"
        // Fades in place. Not a slide: these carry glass, and moving
        // a glass surface mid-animation samples the wrong region.
        visible: opacity > 0.01
        opacity: root.controller.trashActive ? 1 : 0
        Behavior on opacity {
            NumberAnimation {
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }
        height: visible ? implicitHeight : 0
        icon.name: "edit-undo"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.controller.restoreTrash(
                         root.controller.indexForToken(root.targetToken))
    }
    GlassMenuItem {
        text: "Eliminar permanentemente"
        visible: root.controller.trashActive
        height: visible ? implicitHeight : 0
        icon.name: "edit-delete"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.controller.purgeTrash(
                         root.controller.indexForToken(root.targetToken))
    }

    GlassMenuItem {
        text: root.targetDirectory ? "Abrir carpeta" : "Abrir"
        visible: !root.multi && !root.controller.trashActive
        height: visible ? implicitHeight : 0
        icon.name: root.targetDirectory ? "folder-open" : "text-x-generic"
        icon.source: CelestinaTheme.fallbackIcon(
                         root.targetDirectory ? "folder" : "file")
        onTriggered: root.controller.activateToken(root.targetToken)
    }

    GlassMenuItem {
        text: "Abrir con…"
        visible: !root.targetDirectory && !root.multi && !root.controller.trashActive
        height: visible ? implicitHeight : 0
        icon.name: "system-run"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.controller.openWith(root.targetPath)
    }

    GlassMenuItem {
        text: "Enviar al móvil"
        // Only for a single file, and only when a phone is connected.
        visible: !root.targetDirectory && !root.multi
                 && !root.controller.trashActive && root.controller.phoneNames.length > 0
        height: visible ? implicitHeight : 0
        icon.name: "phone"
        icon.source: CelestinaTheme.fallbackIcon("phone")
        onTriggered: root.controller.sendToPhone(root.targetPath)
    }

    GlassMenuItem {
        text: "Abrir en pestaña nueva"
        visible: root.targetDirectory && !root.multi && !root.controller.trashActive
        height: visible ? implicitHeight : 0
        icon.name: "tab-new"
        icon.source: CelestinaTheme.fallbackIcon("folder")
        onTriggered: root.newTabRequested(root.targetPath, true)
    }

    GlassMenuItem {
        text: "Añadir a marcadores"
        visible: root.targetDirectory && !root.multi && !root.controller.trashActive
        height: visible ? implicitHeight : 0
        icon.name: "bookmark-new"
        icon.source: CelestinaTheme.fallbackIcon("folder")
        onTriggered: root.controller.addBookmark(root.targetPath)
    }

    GlassMenuItem {
        text: root.panel.isFavorite(root.targetPath)
              ? "Quitar de favoritos" : "Añadir a favoritos"
        visible: !root.multi && !root.controller.trashActive
        height: visible ? implicitHeight : 0
        // The bundled star, deliberately not the theme's: this entry has
        // to read as the same mark the badge draws on the tile, and not
        // every theme carries both a filled and an outline star.
        icon.source: CelestinaTheme.fallbackIcon(
                         root.panel.isFavorite(root.targetPath)
                         ? "star" : "star-outline")
        onTriggered: root.controller.toggleFavorite(root.targetPath)
    }

    GlassMenuItem {
        text: "Renombrar"
        visible: !root.multi && !root.controller.trashActive
        height: visible ? implicitHeight : 0
        icon.name: "edit-rename"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.namePrompt.openRename(root.targetPath, root.targetName)
    }

    GlassMenuItem {
        text: "Renombrar " + root.actingCount + " elementos…"
        visible: root.multi && !root.controller.trashActive
        height: visible ? implicitHeight : 0
        icon.name: "edit-rename"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.batchRename.open(
                         root.panel.operativePaths(root.targetToken,
                                                   root.targetPath))
    }

    GlassMenuItem {
        text: root.multi
              ? "Copiar " + root.actingCount + " elementos"
              : "Copiar"
        visible: !root.controller.trashActive
        height: visible ? implicitHeight : 0
        icon.name: "edit-copy"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.panel.copySelection(
                         root.targetToken, root.targetPath, false)
    }

    GlassMenuItem {
        text: root.multi
              ? "Cortar " + root.actingCount + " elementos"
              : "Cortar"
        visible: !root.controller.trashActive
        height: visible ? implicitHeight : 0
        icon.name: "edit-cut"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.panel.copySelection(
                         root.targetToken, root.targetPath, true)
    }

    GlassMenuItem {
        text: root.multi
              ? "Enviar " + root.actingCount + " a la papelera"
              : "Enviar a la papelera"
        visible: !root.controller.trashActive
        height: visible ? implicitHeight : 0
        icon.name: "user-trash"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.panel.trashSelection(
                         root.targetToken, root.targetPath)
    }

    GlassMenuItem {
        text: "Cambiar icono…"
        visible: !root.multi && !root.controller.trashActive
        height: visible ? implicitHeight : 0
        icon.name: "preferences-desktop-icons"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.iconPicker.openFor(root.targetPath, root.targetDirectory)
    }

    IconAccentMenu {
        title: "Color del icono"
        icon.name: "paintbrush"
        icon.source: CelestinaTheme.fallbackIcon("paintbrush")
        backdropSource: root.backdropSource
        enabled: !root.multi && !root.controller.trashActive
        currentKey: root.panel.customIconAccent(root.targetPath)
        onAccentSelected: function(accentKey) {
            root.controller.setCustomIconAccent(root.targetPath, accentKey)
            root.close()
        }
    }

    GlassMenuItem {
        text: "Propiedades"
        visible: !root.multi
        height: visible ? implicitHeight : 0
        icon.name: "document-properties"
        icon.source: CelestinaTheme.fallbackIcon("file")
        onTriggered: root.controller.openProperties(root.targetPath)
    }

    MenuSeparator {
        contentItem: Rectangle {
            implicitHeight: 1
            color: CelestinaTheme.divider
        }
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
