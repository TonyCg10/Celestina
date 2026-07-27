import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import org.celestina.siderita 1.0
import org.celestina.siderita.internal 1.0

// ─── FolderView ───────────────────────────────────────────────────────────────
// Una vista de carpeta independiente: migas de pan y búsqueda, la lista o la
// rejilla, la selección múltiple, los menús contextuales de entrada, carpeta y
// orden, y los atajos de navegación de la pestaña. Tiene su propio controlador,
// así que dos pestañas no comparten ni la ubicación ni el historial.
//
// Era un `component` dentro de la ventana principal, que así llegaba a 6.500
// líneas. Lo único que necesitaba de fuera son cuatro cosas, y ahora las pide
// por su nombre: el fantasma de arrastre, dónde poner los emergentes, la
// ventana anfitriona y a quién avisar cuando quiere una pestaña nueva.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    // El pie flota sobre el contenido: hay cristal que enseñar mientras
    // quede lista por debajo. Al llegar al final no queda nada detrás y las
    // pastillas vuelven a su relleno liso.
    readonly property Item bottomView: mainPanel.viewMode === "grid"
                                       ? fileGrid : fileList
    readonly property bool bottomFloating:
            bottomView && bottomView.contentHeight > bottomView.height
            && !bottomView.atYEnd

    property Item ghost
    property Item overlayParent
    // La ventana que hospeda este documento: de ella vienen el modelo de
    // pestañas y las seis escalas de tamaño, que son de la ventana y no de la
    // pestaña — el menú de tamaños vive aquí dentro pero ajusta a todas.
    property var hostWindow
    property bool active: false
    property alias tabController: controller
    signal requestNewTab(string path, bool foreground)

    SideritaController {
        id: controller
    }

    // Native role model fed by the controller's rowsReady signal; the
    // list/grid bind to it instead of a QStringList of names.
    SideritaEntryModel {
        id: entryModel
    }
    // La última ubicación dibujada, para distinguir un cambio de sitio de un
    // refresco del mismo (ruta + los modos que cambian la lista sin cambiar la
    // ruta: papelera, recientes, búsqueda).
    property string renderedKey: ""

    Connections {
        target: controller
        function onRowsReady(names, tokens, kinds, subtitles, paths, sections, sizes, dates) {
            var view = mainPanel.viewMode === "grid" ? fileGrid : fileList
            var key = controller.currentPath + "|" + controller.trashActive + "|"
                    + controller.recentActive + "|" + controller.searchActive + "|"
                    + controller.searchQuery
            var samePlace = key === root.renderedKey
            var savedY = view.contentY

            entryModel.setRows(names, tokens, kinds, subtitles, paths, sections, sizes, dates)
            root.renderedKey = key

            // setRows reinicia el modelo, y un reset manda la vista al top (que
            // con el margen superior es contentY = -topMargin, no 0). En la misma
            // ubicación —un refresco del watch— eso te arranca de donde leías, así
            // que le devolvemos su contentY (la vista lo acota a rango sola). Al
            // navegar no se toca: ir al top es lo correcto para una carpeta nueva.
            if (samePlace)
                view.contentY = savedY
        }
    }

    // ── Quick-look preview state (spacebar) ──────────────────────────
    // The overlay (below) previews whatever entry is selected; ↑/↓ while it
    // is open step the selection so the preview browses the folder without
    // closing. On close, focus returns to the active view so the keyboard
    // keeps working.
    property bool quickLookOpen: false
    onQuickLookOpenChanged: if (!quickLookOpen) {
        if (mainPanel.viewMode === "grid")
            fileGrid.forceActiveFocus()
        else
            fileList.forceActiveFocus()
    }
    // Devuelve el foco a la vista activa. Lo usan los diálogos al cerrarse
    // (viven en sus propios ficheros y ya no alcanzan `fileList`); antes fijaban
    // la lista aunque estuviera activa la rejilla, así que ahora el teclado
    // también responde tras cerrar un diálogo en modo cuadrícula.
    function focusView() {
        if (mainPanel.viewMode === "grid")
            fileGrid.forceActiveFocus()
        else
            fileList.forceActiveFocus()
    }

    function quickLookStep(delta) {
        var n = controller.entryNames.length
        if (n === 0)
            return
        var i = controller.indexForToken(controller.selectedToken)
        var j = Math.max(0, Math.min(n - 1, (i < 0 ? 0 : i) + delta))
        if (mainPanel.viewMode === "grid")
            fileGrid.selectCell(j)
        else
            fileList.selectRow(j)
    }

    // Drop any active search — the live filter and the recursive results —
    // and empty the field, returning the content box to the plain listing.
    // Fired on navigation (so a sidebar/place click lands on a clean folder)
    // and by the mouse Back button.
    function clearSearch() {
        topBar.searchText = ""
        controller.applyQuery("")
        controller.closeSearch()
    }

    // Back undoes wherever you are, in the order you got there: a search
    // bows out first, then the Trash location, and only then does history
    // move. Trash is a place you can be but not a folder in history, so
    // without this the only way out was the sidebar.
    readonly property bool canGoBackOrLeave:
            !controller.loading
            && (topBar.searchText.length > 0 || controller.searchActive
                || controller.trashActive || controller.recentActive
                || controller.canGoBack)

    function goBackOrLeave() {
        if (controller.loading)
            return
        if (topBar.searchText.length > 0 || controller.searchActive)
            clearSearch()
        else if (controller.trashActive)
            controller.closeTrash()
        else if (controller.recentActive)
            controller.closeRecent()
        else if (controller.canGoBack)
            controller.goBack()
    }

    // Per-tab shortcuts: only the active (visible) tab responds, so the
    // same sequence across tabs is never ambiguous.
    Shortcut {
        sequence: "Alt+Left"
        enabled: root.active && root.canGoBackOrLeave
        onActivated: root.goBackOrLeave()
    }

    Shortcut {
        sequence: "Alt+Right"
        enabled: root.active && controller.canGoForward && !controller.loading
        onActivated: controller.goForward()
    }

    Shortcut {
        sequence: "Alt+Up"
        enabled: root.active && controller.canGoUp && !controller.loading
        onActivated: controller.goUp()
    }

    Shortcut {
        sequence: "Ctrl+L"
        enabled: root.active
        onActivated: topBar.beginEditing()
    }

    Shortcut {
        sequence: "Ctrl+F"
        enabled: root.active
        onActivated: topBar.focusSearch()
    }

    Shortcut {
        sequence: "Ctrl+H"
        enabled: root.active && !controller.loading
        onActivated: controller.toggleHidden()
    }

    Shortcut {
        sequence: "F5"
        enabled: root.active && !controller.loading
        onActivated: controller.refresh()
    }

    // Write verbs act on the focused entry (topBar.activeView.currentIndex).
    Shortcut {
        sequence: "F2"
        enabled: root.active && !controller.loading && !controller.opRunning
        onActivated: {
            const i = topBar.activeView.currentIndex
            if (i >= 0)
                namePrompt.openRename(controller.entryPath(i),
                                      controller.entryNames[i])
        }
    }

    Shortcut {
        sequence: "Delete"
        enabled: root.active && !controller.loading && !controller.opRunning
                 && !controller.trashActive
        onActivated: {
            // Act on the whole selection — whatever set it (marquee,
            // Ctrl/Shift-click or a single click). The keyboard cursor
            // (currentIndex) is only a fallback, because a marquee selection
            // never moves it, which is why Delete used to miss it.
            var p = mainPanel.selectedPaths()
            if (p.length > 1)
                controller.trashPaths(p)
            else if (p.length === 1)
                controller.trashPath(p[0])
            else {
                const i = topBar.activeView.currentIndex
                if (i >= 0)
                    controller.trashPath(controller.entryPath(i))
            }
        }
    }

    Shortcut {
        sequences: [StandardKey.Copy]
        enabled: root.active && !controller.trashActive
        onActivated: {
            // Selection first (marquee included), cursor only as fallback.
            var p = mainPanel.selectedPaths()
            if (p.length > 1)
                controller.copyPathsToClipboard(p, false)
            else if (p.length === 1)
                controller.copyToClipboard(p[0], false)
            else {
                const i = topBar.activeView.currentIndex
                if (i >= 0)
                    controller.copyToClipboard(controller.entryPath(i), false)
            }
        }
    }

    Shortcut {
        sequences: [StandardKey.Cut]
        enabled: root.active && !controller.trashActive
        onActivated: {
            // Selection first (marquee included), cursor only as fallback.
            var p = mainPanel.selectedPaths()
            if (p.length > 1)
                controller.copyPathsToClipboard(p, true)
            else if (p.length === 1)
                controller.copyToClipboard(p[0], true)
            else {
                const i = topBar.activeView.currentIndex
                if (i >= 0)
                    controller.copyToClipboard(controller.entryPath(i), true)
            }
        }
    }

    Shortcut {
        sequences: [StandardKey.Paste]
        enabled: root.active && controller.canPaste && !controller.opRunning
        onActivated: controller.paste()
    }

    Shortcut {
        sequences: [StandardKey.Undo]
        enabled: root.active && controller.canUndo && !controller.loading && !controller.opRunning
        onActivated: controller.undo()
    }

    TapHandler {
        id: historyMouseButtons

        enabled: root.active
        acceptedButtons: Qt.BackButton | Qt.ForwardButton
        gesturePolicy: TapHandler.ReleaseWithinBounds

        onTapped: function(eventPoint, button) {
            if (controller.loading)
                return

            if (button === Qt.BackButton) {
                root.goBackOrLeave()
            } else if (button === Qt.ForwardButton
                       && controller.canGoForward) {
                controller.goForward()
            }
        }
    }

    Rectangle {
        id: mainPanel

        property string viewMode: "list"   // "list" | "grid"

        // Restore the last-used view mode on open and persist a change
        // (list⇄grid). The size scales are window-level and independent
        // (root.hostWindow.contentIconScale / root.hostWindow.contentTextScale).
        Component.onCompleted: {
            viewMode = controller.savedViewMode()
            rebuildFolderTypeIcons()
        }
        // Picking a view is both a new default and a statement about this
        // folder: the global keeps folders you have never arranged looking
        // the way you last chose, and this folder now remembers its own.
        function persist() {
            controller.saveViewMode(viewMode)
            controller.rememberViewMode(viewMode)
        }

        // A folder that remembers a view wins over the global default when
        // it opens. `folderViewMode` is empty for folders never arranged,
        // which leaves whatever the user is currently looking at alone.
        Connections {
            target: controller
            function onFolderViewModeChanged() {
                const mode = controller.folderViewMode
                if (mode.length > 0 && mode !== mainPanel.viewMode)
                    mainPanel.viewMode = mode
            }
        }

        // The content row/cell (the "selection square") sizes to fit
        // whichever is taller — the icon or the independently-scaled text —
        // so the two size sliders never clip one another.
        readonly property int listRowHeight: Math.max(
            Math.round(CelestinaTheme.glyphTile * root.hostWindow.contentIconScale),
            Math.round((CelestinaTheme.fontBody + CelestinaTheme.fontCaption)
                       * 1.35 * root.hostWindow.contentTextScale)) + 16
        readonly property int gridCellWidth: Math.round(
            104 * Math.max(root.hostWindow.contentIconScale, root.hostWindow.contentTextScale))
        readonly property int gridCellHeight:
            Math.round(72 * root.hostWindow.contentIconScale) + 8
            + Math.round(CelestinaTheme.fontCaption * 2.9 * root.hostWindow.contentTextScale) + 20

        // ── Multi-selection (token-keyed, so it survives sort/filter) ────
        property var selectedTokens: ({})
        property int selectionCount: 0
        property string anchorToken: ""
        // Mirror the count to the controller so the window-scope info box
        // (which only reaches the active tab's controller) can read it.
        onSelectionCountChanged: controller.selectionCount = selectionCount

        function isSelected(token) {
            return token.length > 0 && selectedTokens[token] === true
        }
        // The media class of a file by extension — "image", "video",
        // "audio" or "" — driving its themed icon and whether it asks the
        // thumbnail provider (which reuses any cached thumbnail for any type,
        // but only generates images itself; video/audio come from a producer).
        function mediaKind(n) {
            if (/\.(png|jpe?g|gif|webp|bmp|ico|tiff?|avif|jxl|heic|heif)$/i.test(n))
                return "image"
            if (/\.(mp4|mkv|webm|mov|avi|m4v|mpe?g|wmv|flv|3gp|ogv|ts)$/i.test(n))
                return "video"
            if (/\.(mp3|flac|ogg|oga|opus|m4a|aac|wav|wma|aiff?|mka)$/i.test(n))
                return "audio"
            return ""
        }
        // Map each XDG user directory's PATH to its freedesktop folder-type
        // icon, so Documentos / Descargas / Música / … show their own glyph
        // in the content view, not the generic folder. Rebuilt on open; the
        // paths are user-level and stable.
        property var folderTypeIcons: ({})
        function rebuildFolderTypeIcons() {
            var defs = { DESKTOP: "folder-desktop", DOCUMENTS: "folder-documents",
                         DOWNLOAD: "folder-download", MUSIC: "folder-music",
                         PICTURES: "folder-pictures", VIDEOS: "folder-videos",
                         PUBLICSHARE: "folder-publicshare", TEMPLATES: "folder-templates" }
            var m = {}
            for (var k in defs) {
                var p = controller.placePath(k)
                if (p.length > 0)
                    m[p] = defs[k]
            }
            folderTypeIcons = m
        }
        function folderIcon(path) {
            return (path && folderTypeIcons[path]) ? folderTypeIcons[path] : "folder"
        }
        // User-chosen per-path icon overrides (Cambiar icono…), folded from
        // the controller's `path\ticon` list into a map. A binding, not a
        // hand-rolled rebuild: it re-runs by itself whenever the list
        // changes, so a new icon shows at once instead of at the next start.
        readonly property var customIcons: {
            var m = {}
            var entries = controller.customIconEntries
            for (var i = 0; i < entries.length; i++) {
                var cut = entries[i].indexOf("\t")
                if (cut > 0)
                    m[entries[i].substring(0, cut)] = entries[i].substring(cut + 1)
            }
            return m
        }
        // Starred paths ("Añadir a favoritos"), folded into a set for O(1)
        // lookup from every delegate. Same shape as customIcons: a binding,
        // so a star appears the moment it is set.
        readonly property var favorites: {
            var s = {}
            var entries = controller.favoriteEntries
            for (var i = 0; i < entries.length; i++) {
                var cut = entries[i].indexOf("\t")
                s[cut > 0 ? entries[i].substring(0, cut) : entries[i]] = true
            }
            return s
        }
        function isFavorite(path) {
            return path.length > 0 && favorites[path] === true
        }

        // The themed icon a non-thumbnailed entry shows — a user override if
        // set, else a media-type icon (video/audio/image), a type-specific
        // folder, else generic.
        function mediaIconName(kind, media, path) {
            if (path && customIcons[path])
                return customIcons[path]
            return kind === "directory" ? folderIcon(path)
                 : kind === "symlink" ? "emblem-symbolic-link"
                 : media === "image" ? "image-x-generic"
                 : media === "video" ? "video-x-generic"
                 : media === "audio" ? "audio-x-generic"
                 : "text-x-generic"
        }
        function clearSelection() {
            selectedTokens = ({})
            selectionCount = 0
        }
        function selectOnly(token) {
            var s = {}
            s[token] = true
            selectedTokens = s
            selectionCount = 1
            anchorToken = token
        }
        function toggleSelection(token) {
            var s = Object.assign({}, selectedTokens)
            if (s[token])
                delete s[token]
            else
                s[token] = true
            selectedTokens = s
            selectionCount = Object.keys(s).length
            anchorToken = token
        }
        function selectRange(toIndex) {
            var anchorIdx = controller.indexForToken(anchorToken)
            if (anchorIdx < 0)
                anchorIdx = toIndex
            var s = {}
            for (var i = Math.min(anchorIdx, toIndex);
                 i <= Math.max(anchorIdx, toIndex); i++) {
                var t = controller.entryToken(i)
                if (t.length > 0)
                    s[t] = true
            }
            selectedTokens = s
            selectionCount = Object.keys(s).length
        }
        function selectAll() {
            var s = {}
            for (var i = 0; i < controller.entryNames.length; i++) {
                var t = controller.entryToken(i)
                if (t.length > 0)
                    s[t] = true
            }
            selectedTokens = s
            selectionCount = Object.keys(s).length
        }
        // Selection = base plus every item sampled inside the marquee rect
        // (x0,y0..x1,y1 in the view's viewport coordinates).
        function selectRectFrom(base, view, x0, y0, x1, y1) {
            var s = Object.assign({}, base)
            var cx = view.contentX
            var cy = view.contentY
            for (var y = y0; y <= y1; y += 10) {
                for (var x = x0; x <= x1; x += 24) {
                    var idx = view.indexAt(x + cx, y + cy)
                    if (idx >= 0) {
                        var t = controller.entryToken(idx)
                        if (t.length > 0)
                            s[t] = true
                    }
                }
            }
            selectedTokens = s
            selectionCount = Object.keys(s).length
        }

        // Paths of every currently-selected entry that is still visible in
        // this view (a token filtered away resolves to -1 and is skipped, so
        // a verb only ever touches what the user can see).
        function selectedPaths() {
            var out = []
            for (var t in selectedTokens) {
                if (selectedTokens[t] === true) {
                    var idx = controller.indexForToken(t)
                    if (idx >= 0)
                        out.push(controller.entryPath(idx))
                }
            }
            return out
        }
        // The set a verb should act on: the whole selection when the
        // right-clicked / focused entry is part of a multi-selection, else
        // just that one entry.
        function operativePaths(primaryToken, primaryPath) {
            if (selectionCount > 1 && isSelected(primaryToken))
                return selectedPaths()
            return [primaryPath]
        }
        function actingCount(primaryToken) {
            return (selectionCount > 1 && isSelected(primaryToken))
                   ? selectionCount : 1
        }
        function copySelection(primaryToken, primaryPath, cut) {
            var p = operativePaths(primaryToken, primaryPath)
            if (p.length > 1)
                controller.copyPathsToClipboard(p, cut)
            else
                controller.copyToClipboard(primaryPath, cut)
        }
        function trashSelection(primaryToken, primaryPath) {
            var p = operativePaths(primaryToken, primaryPath)
            if (p.length > 1)
                controller.trashPaths(p)
            else
                controller.trashPath(primaryPath)
        }
        // Converts dropped file:// URLs to local paths (percent-decoded),
        // skipping any non-file URL, for controller.dropUris.
        function urlsToPaths(urls) {
            var out = []
            for (var i = 0; i < urls.length; i++) {
                var u = urls[i].toString()
                if (u.indexOf("file://") === 0)
                    out.push(decodeURIComponent(u.substring(7)))
            }
            return out
        }
        // Shift forces a move; the default for a cross-application drop is the
        // safe copy.
        function dropIsMove(drop) {
            return (drop.keyboardModifiers & Qt.ShiftModifier) !== 0
        }
        // True if the drag is an internal Siderita entry (a file/folder being
        // dragged within the view), as opposed to external file URLs.
        function isEntryDrag(drag) {
            return drag.keys.indexOf("siderita-entry") >= 0
        }
        // Land a drop into `destPath` ("" = current folder). An internal
        // entry drag is detected by our key FIRST (it now also carries a
        // uri-list for external apps, so hasUrls is true too) and defaults to
        // move (Ctrl = copy); a genuinely external drop uses the URLs and
        // defaults to copy (Shift = move).
        function dropOnto(destPath, drop) {
            if (isEntryDrag(drop) && root.ghost.path.length > 0) {
                var move = (drop.keyboardModifiers & Qt.ControlModifier) === 0
                controller.dropUris([root.ghost.path], destPath, move)
            } else if (drop.hasUrls) {
                controller.dropUris(urlsToPaths(drop.urls), destPath,
                                    dropIsMove(drop))
            }
        }
        // Begin dragging an entry. The drag is Drag.Automatic (so it can also
        // land in other apps as a uri-list), which hands the visual to the
        // compositor — a manually-positioned QML ghost can't follow the
        // cursor under a native drag and just strands itself at 0,0. So we
        // grab the entry's icon into Drag.imageSource and let the platform
        // render it at the pointer, hot-spotted on the icon's centre. If the
        // grab can't start we still activate, so the drag never fails to run.
        function startEntryDrag(entryPath, entryLabel, entryIsDir, glyphItem, handler) {
            root.ghost.beginEntryDrag(entryPath, entryLabel, entryIsDir)
            var started = glyphItem.grabToImage(function(result) {
                if (result) {
                    root.ghost.Drag.imageSource = result.url
                    root.ghost.Drag.hotSpot = Qt.point(glyphItem.width / 2,
                                                       glyphItem.height / 2)
                }
                // The grab is a frame late; only start if the press is still
                // down, or a quick release would strand an active drag.
                if (handler.active)
                    root.ghost.Drag.active = true
            })
            if (!started && handler.active)
                root.ghost.Drag.active = true
        }

        Connections {
            target: controller
            function onCurrentPathChanged() {
                mainPanel.clearSelection()
                // Salir de la búsqueda sólo si había una. clearSearch reproyecta
                // el snapshot actual, y en plena navegación ese snapshot es aún
                // el de la carpeta ANTERIOR (el escaneo del destino no ha
                // llegado): reproyectarlo publicaba las filas viejas con la ruta
                // ya cambiada, y la carpeta de antes se repintaba arriba antes de
                // aparecer la nueva. Sin búsqueda activa no hay nada que cerrar.
                if (controller.searchActive)
                    root.clearSearch()
            }
            // Entering or leaving search swaps the whole row set (folder ↔
            // hits, index-keyed vs token-keyed), so drop the old selection.
            function onSearchActiveChanged() { mainPanel.clearSelection() }
        }

        anchors.fill: parent
        radius: CelestinaTheme.radiusLg
        color: CelestinaTheme.surface
        border.width: 1
        border.color: CelestinaTheme.border

        // Bottom control bar: all controls and status render along the
        // bottom of the content box; the list/grid fill from the top.
        // Sin barra: igual que la cabecera, los controles del pie son
        // pastillas sueltas sobre el contenido. La línea que los separaba
        // dibujaba una barra que no existe — cada control ya se delimita
        // solo, y el contenido pasa por debajo entre ellos.
        Item {
            id: bottomBar
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 54
        }

        // Bottom-bar controls flow left-to-right and size to their (scaled)
        // content, so a larger interface text never clips or overlaps them.
        BottomControls {
            id: bottomControls
            x: 16
            anchors.verticalCenter: bottomBar.verticalCenter
            controller: controller
            panel: mainPanel
            bottomView: root.bottomView
            bottomFloating: root.bottomFloating
            overlayParent: root.overlayParent
            sortMenu: sortMenu
            textScale: root.hostWindow.interfaceTextScale
        }

        // Right-click on empty space (behind the views) opens the folder
        // menu; right-clicks that land on an item are handled by the item.
        MouseArea {
            id: emptySpaceMouse
            x: 8
            y: 14
            width: parent.width - 16
            height: parent.height - 68
            acceptedButtons: Qt.RightButton
            onClicked: function(mouse) {
                const point = emptySpaceMouse.mapToItem(
                                root.overlayParent, mouse.x, mouse.y)
                folderMenu.popup(root.overlayParent, point)
            }
        }

        // Accept external file drops into the current folder. Folder rows
        // carry their own DropArea (below) that lands the drop in that folder
        // instead; this one catches empty space and non-folder rows.
        DropArea {
            id: viewDrop
            anchors.fill: parent
            anchors.bottomMargin: 68
            // No z bump: the list/grid (declared after) and their per-folder
            // DropAreas must stack above this, so a drop on a folder lands in
            // that folder and only empty space falls through to here.

            onEntered: function(drag) {
                if (!drag.hasUrls)
                    drag.accepted = false
            }
            onDropped: function(drop) {
                if (!drop.hasUrls)
                    return
                controller.dropUris(mainPanel.urlsToPaths(drop.urls),
                                    "", mainPanel.dropIsMove(drop))
                drop.accept()
            }

            Rectangle {
                anchors.fill: parent
                visible: viewDrop.containsDrag
                color: "transparent"
                border.width: 2
                border.color: CelestinaTheme.accent
                radius: CelestinaTheme.radiusLg
                z: 40
            }
        }

        ListView {
            id: fileList
            x: 8
            y: 14
            width: parent.width - 16
            // Llega hasta el borde: el pie flota encima, no le recorta sitio.
            height: parent.height - 22
            // …y un pie vacío devuelve el sitio por dentro, para que la
            // última fila se pueda leer entera en vez de quedarse bajo las
            // pastillas. Es un pie y no `bottomMargin` porque el margen entra
            // en el cálculo de contentY y monta un bucle de enlace con el
            // desplazamiento que vigila el cristal de la cabecera.
            footer: Item { width: 1; height: 46 }
            // The list backs three modes: plain "list", the "details"
            // columns (same rows, a different delegate body), and search
            // (which always uses the sectioned list — a grid can't carry
            // section headers).
            // Search results ride the same model as a folder, so they honour
            // the chosen view like the Trash does — the one exception is the
            // details columns, which a hit has no size or date for.
            visible: mainPanel.viewMode !== "grid"
            readonly property bool detailsMode: mainPanel.viewMode === "details"
                                                && !controller.searchActive
            // Column widths shared by the details rows and their header
            // (name fills the rest); they track the content text scale.
            readonly property int colSizeW: Math.round(92 * root.hostWindow.contentTextScale)
            readonly property int colDateW: Math.round(150 * root.hostWindow.contentTextScale)
            readonly property int colTypeW: Math.round(96 * root.hostWindow.contentTextScale)
            // Where the name column starts — past the row's icon glyph — so
            // the header lines up with the rows.
            readonly property int detailsNameX: 14
                    + Math.round(CelestinaTheme.glyphTile * root.hostWindow.contentIconScale) + 12
            model: entryModel
            clip: true
            spacing: 2
            reuseItems: true
            cacheBuffer: 420
            topMargin: 62 + (tabBar.visible ? tabBar.height + 8 : 0)
                     + (searchBar.visible ? searchBar.height + 8 : 0)
                     + (trashHeader.visible ? trashHeader.height + 8 : 0)
                     + (recentHeader.visible ? recentHeader.height + 8 : 0)
                     + (fileList.detailsMode ? detailsHeader.height + 8 : 0)
            boundsBehavior: Flickable.StopAtBounds

            // Empty for a plain folder listing (no headers); set to the group
            // label for search results.
            section.property: "section"
            section.criteria: ViewSection.FullString
            section.delegate: Item {
                id: sectionHeader
                required property string section
                width: fileList.width
                height: sectionHeader.section.length > 0
                        ? Math.round(CelestinaTheme.fontMini * root.hostWindow.contentTextScale) + 22
                        : 0
                visible: sectionHeader.section.length > 0

                Text {
                    x: 14
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 6
                    text: sectionHeader.section.toUpperCase()
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontMini * root.hostWindow.contentTextScale)
                    font.letterSpacing: 1.4
                    font.weight: CelestinaTheme.weightDemiBold
                }
            }
            activeFocusOnTab: true
            keyNavigationEnabled: false
            currentIndex: -1

            Connections {
                target: entryModel
                // The model reset clears currentIndex; restore it from the
                // controller's selected token.
                function onModelReset() {
                    fileList.currentIndex = controller.indexForToken(
                                controller.selectedToken)
                }
            }

            function selectRow(i) {
                if (i < 0 || i >= count)
                    return
                currentIndex = i
                const t = controller.entryToken(i)
                mainPanel.selectOnly(t)
                controller.selectToken(t)
                positionViewAtIndex(i, ListView.Contain)
            }

            function pageStep() {
                return Math.max(
                    1, Math.floor(height / (mainPanel.listRowHeight + spacing)))
            }

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape && controller.searchActive) {
                    controller.closeSearch()
                    event.accepted = true
                    return
                }
                if (count === 0)
                    return

                const i = currentIndex

                if (event.key === Qt.Key_Down) {
                    selectRow(Math.min(count - 1, i + 1))
                    event.accepted = true
                } else if (event.key === Qt.Key_Up) {
                    selectRow(i < 0 ? count - 1 : Math.max(0, i - 1))
                    event.accepted = true
                } else if (event.key === Qt.Key_Home) {
                    selectRow(0)
                    event.accepted = true
                } else if (event.key === Qt.Key_End) {
                    selectRow(count - 1)
                    event.accepted = true
                } else if (event.key === Qt.Key_PageDown) {
                    selectRow(Math.min(count - 1, (i < 0 ? 0 : i) + pageStep()))
                    event.accepted = true
                } else if (event.key === Qt.Key_PageUp) {
                    selectRow(Math.max(0, (i < 0 ? 0 : i) - pageStep()))
                    event.accepted = true
                } else if (event.key === Qt.Key_Backspace) {
                    if (controller.canGoUp && !controller.loading)
                        controller.goUp()
                    event.accepted = true
                } else if (i >= 0
                           && (event.key === Qt.Key_Return
                               || event.key === Qt.Key_Enter)) {
                    controller.activateToken(controller.entryToken(i))
                    event.accepted = true
                } else if (event.key === Qt.Key_Space
                           && controller.selectedToken.length > 0) {
                    // Quick-look the selected entry (Space toggles it shut
                    // again from inside the overlay's own key handler).
                    root.quickLookOpen = true
                    event.accepted = true
                } else if (event.modifiers === Qt.NoModifier
                           && event.text.length === 1
                           && event.text !== " "
                           && event.text >= " ") {
                    // type-ahead: jump to the next entry starting with the char
                    const ch = event.text.toLowerCase()
                    const start = i < 0 ? -1 : i
                    for (let k = 1; k <= count; k++) {
                        const j = (start + k) % count
                        const name = controller.entryNames[j]
                        if (name && name.toLowerCase().indexOf(ch) === 0) {
                            selectRow(j)
                            break
                        }
                    }
                    event.accepted = true
                }
            }

            delegate: FolderRowDelegate {
                panel: mainPanel
                controller: controller
                view: fileList
                hostWindow: root.hostWindow
                ghost: root.ghost
                overlayParent: root.overlayParent
                onNewTabRequested: function(path, foreground) {
                    root.requestNewTab(path, foreground)
                }
                onContextMenuRequested: function(token, name, isDir, path, x, y) {
                    entryMenu.targetToken = token
                    entryMenu.targetName = name
                    entryMenu.targetDirectory = isDir
                    entryMenu.targetPath = path
                    entryMenu.popup(root.overlayParent, Qt.point(x, y))
                }
            }

            ScrollBar.vertical: ScrollBar {
                policy: ScrollBar.AsNeeded
            }
        }

        GridView {
            id: fileGrid
            x: 8
            y: 14
            width: parent.width - 16
            // Llega hasta el borde: el pie flota encima, no le recorta sitio.
            height: parent.height - 22
            // …y un pie vacío devuelve el sitio por dentro, para que la
            // última fila se pueda leer entera en vez de quedarse bajo las
            // pastillas. Es un pie y no `bottomMargin` porque el margen entra
            // en el cálculo de contentY y monta un bucle de enlace con el
            // desplazamiento que vigila el cristal de la cabecera.
            footer: Item { width: 1; height: 46 }
            visible: mainPanel.viewMode === "grid"
            model: entryModel
            clip: true
            // Stretch the columns to fill the width: fit as many natural-size
            // cells as possible, then divide the width evenly among them, so
            // the leftover never piles up as one gap on the right.
            readonly property int cols: Math.max(1, Math.floor(width / mainPanel.gridCellWidth))
            cellWidth: Math.floor(width / cols)
            cellHeight: mainPanel.gridCellHeight
            cacheBuffer: 480
            topMargin: 62 + (tabBar.visible ? tabBar.height + 8 : 0)
                     + (searchBar.visible ? searchBar.height + 8 : 0)
                     + (trashHeader.visible ? trashHeader.height + 8 : 0)
                     + (recentHeader.visible ? recentHeader.height + 8 : 0)
            boundsBehavior: Flickable.StopAtBounds
            activeFocusOnTab: true
            keyNavigationEnabled: false
            currentIndex: -1

            Connections {
                target: entryModel
                function onModelReset() {
                    fileGrid.currentIndex = controller.indexForToken(
                                controller.selectedToken)
                }
            }

            function columns() {
                return cols
            }

            function selectCell(i) {
                if (i < 0 || i >= count)
                    return
                currentIndex = i
                const t = controller.entryToken(i)
                mainPanel.selectOnly(t)
                controller.selectToken(t)
                positionViewAtIndex(i, GridView.Contain)
            }

            function pageStep() {
                const rows = Math.max(1, Math.floor(height / cellHeight))
                return rows * columns()
            }

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape && controller.searchActive) {
                    controller.closeSearch()
                    event.accepted = true
                    return
                }
                if (count === 0)
                    return

                const i = currentIndex
                const cols = columns()

                if (event.key === Qt.Key_Right) {
                    selectCell(Math.min(count - 1, i + 1))
                    event.accepted = true
                } else if (event.key === Qt.Key_Left) {
                    selectCell(i < 0 ? count - 1 : Math.max(0, i - 1))
                    event.accepted = true
                } else if (event.key === Qt.Key_Down) {
                    selectCell(i < 0 ? 0 : Math.min(count - 1, i + cols))
                    event.accepted = true
                } else if (event.key === Qt.Key_Up) {
                    selectCell(i < 0 ? count - 1 : Math.max(0, i - cols))
                    event.accepted = true
                } else if (event.key === Qt.Key_Home) {
                    selectCell(0)
                    event.accepted = true
                } else if (event.key === Qt.Key_End) {
                    selectCell(count - 1)
                    event.accepted = true
                } else if (event.key === Qt.Key_PageDown) {
                    selectCell(Math.min(count - 1, (i < 0 ? 0 : i) + pageStep()))
                    event.accepted = true
                } else if (event.key === Qt.Key_PageUp) {
                    selectCell(Math.max(0, (i < 0 ? 0 : i) - pageStep()))
                    event.accepted = true
                } else if (event.key === Qt.Key_Backspace) {
                    if (controller.canGoUp && !controller.loading)
                        controller.goUp()
                    event.accepted = true
                } else if (i >= 0
                           && (event.key === Qt.Key_Return
                               || event.key === Qt.Key_Enter)) {
                    controller.activateToken(controller.entryToken(i))
                    event.accepted = true
                } else if (event.key === Qt.Key_Space
                           && controller.selectedToken.length > 0) {
                    // Quick-look the selected entry (Space toggles it shut
                    // again from inside the overlay's own key handler).
                    root.quickLookOpen = true
                    event.accepted = true
                } else if (event.modifiers === Qt.NoModifier
                           && event.text.length === 1
                           && event.text !== " "
                           && event.text >= " ") {
                    // type-ahead: jump to the next entry starting with the char
                    const ch = event.text.toLowerCase()
                    const start = i < 0 ? -1 : i
                    for (let k = 1; k <= count; k++) {
                        const j = (start + k) % count
                        const name = controller.entryNames[j]
                        if (name && name.toLowerCase().indexOf(ch) === 0) {
                            selectCell(j)
                            break
                        }
                    }
                    event.accepted = true
                }
            }

            delegate: FolderCellDelegate {
                panel: mainPanel
                controller: controller
                view: fileGrid
                hostWindow: root.hostWindow
                ghost: root.ghost
                overlayParent: root.overlayParent
                onNewTabRequested: function(path, foreground) {
                    root.requestNewTab(path, foreground)
                }
                onContextMenuRequested: function(token, name, isDir, path, x, y) {
                    entryMenu.targetToken = token
                    entryMenu.targetName = name
                    entryMenu.targetDirectory = isDir
                    entryMenu.targetPath = path
                    entryMenu.popup(root.overlayParent, Qt.point(x, y))
                }
            }

            ScrollBar.vertical: ScrollBar {
                policy: ScrollBar.AsNeeded
            }
        }

        // Left-drag on empty space draws a marquee selection zone. Presses
        // that land on an item are passed through to the item's handler.
        MouseArea {
            id: marquee
            x: 8
            y: 14
            width: parent.width - 16
            height: parent.height - 68
            acceptedButtons: Qt.LeftButton
            preventStealing: true

            property bool dragging: false
            property real ox: 0
            property real oy: 0
            property real cx: 0
            property real cy: 0
            property var base: ({})
            readonly property Item view: topBar.activeView

            onPressed: function(mouse) {
                const idx = view.indexAt(mouse.x + view.contentX,
                                         mouse.y + view.contentY)
                if (idx >= 0) {
                    mouse.accepted = false   // over an item → item handles it
                    return
                }
                fileList.forceActiveFocus()
                base = (mouse.modifiers & Qt.ControlModifier)
                       ? Object.assign({}, mainPanel.selectedTokens)
                       : {}
                if (!(mouse.modifiers & Qt.ControlModifier))
                    mainPanel.clearSelection()
                dragging = false
                ox = mouse.x; oy = mouse.y
                cx = mouse.x; cy = mouse.y
            }
            onPositionChanged: function(mouse) {
                cx = mouse.x; cy = mouse.y
                if (!dragging && (Math.abs(cx - ox) > 4
                                  || Math.abs(cy - oy) > 4))
                    dragging = true
                if (dragging)
                    mainPanel.selectRectFrom(marquee.base, marquee.view,
                        Math.min(ox, cx), Math.min(oy, cy),
                        Math.max(ox, cx), Math.max(oy, cy))
            }
            onReleased: function(mouse) { marquee.dragging = false }

            Rectangle {
                visible: marquee.dragging
                x: Math.min(marquee.ox, marquee.cx)
                y: Math.min(marquee.oy, marquee.cy)
                width: Math.abs(marquee.cx - marquee.ox)
                height: Math.abs(marquee.cy - marquee.oy)
                radius: 3
                color: Qt.rgba(CelestinaTheme.accent.r, CelestinaTheme.accent.g,
                               CelestinaTheme.accent.b, 0.18)
                border.width: 1
                border.color: CelestinaTheme.accent
            }
        }

        Column {
            anchors.centerIn: fileList
            spacing: 8
            visible: !controller.loading
                     && controller.errorText.length === 0
                     && controller.entryNames.length === 0
                     && !controller.searchRunning

            readonly property bool searchEmpty: controller.searchActive
                                                || controller.query.length > 0

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: parent.searchEmpty ? "Sin coincidencias" : "Carpeta vacía"
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontTitle
                font.weight: CelestinaTheme.weightMedium
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: parent.searchEmpty
                      ? "Prueba con otra búsqueda."
                      : "No hay elementos que mostrar."
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontLabel
            }
        }

        Rectangle {
            id: errorBanner
            x: 16
            // Float above the bottom control bar, not over the breadcrumb.
            anchors.bottom: bottomBar.top
            anchors.bottomMargin: 8
            width: parent.width - 32
            height: errorText.implicitHeight + 22
            radius: CelestinaTheme.radiusSm
            visible: controller.errorText.length > 0
            color: CelestinaTheme.dangerFill
            border.width: 1
            border.color: CelestinaTheme.dangerBorder
            z: 3

            Text {
                id: errorText
                anchors.fill: parent
                anchors.margins: 11
                text: controller.errorText
                color: CelestinaTheme.dangerText
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontLabel
                wrapMode: Text.Wrap
            }
        }

        // Feedback from a write operation (create / rename / trash / paste);
        // cleared on the next operation or navigation.
        Rectangle {
            id: opErrorBanner
            x: 16
            // Stacks upward from the bottom bar, above any general error banner.
            anchors.bottom: errorBanner.visible ? errorBanner.top : bottomBar.top
            anchors.bottomMargin: 8
            width: parent.width - 32
            height: opErrorText.implicitHeight + 22
            radius: CelestinaTheme.radiusSm
            visible: controller.opError.length > 0
            color: CelestinaTheme.dangerFill
            border.width: 1
            border.color: CelestinaTheme.dangerBorder
            z: 4

            Text {
                id: opErrorText
                anchors.fill: parent
                anchors.margins: 11
                text: controller.opError
                color: CelestinaTheme.dangerText
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontLabel
                wrapMode: Text.Wrap
            }
        }

        // Progress surface for a running copy / move: current entry, a
        // determinate bar over the top-level count, bytes copied and a
        // cancel button that trips the worker's cancellation token.
        Rectangle {
            id: opProgressCard
            x: 16
            // Above the error banners, still stacking up from the bottom bar.
            anchors.bottom: opErrorBanner.visible
                            ? opErrorBanner.top
                            : (errorBanner.visible ? errorBanner.top : bottomBar.top)
            anchors.bottomMargin: 8
            width: parent.width - 32
            height: 62
            radius: CelestinaTheme.radiusSm
            visible: controller.opRunning
            color: CelestinaTheme.surface
            border.width: 1
            border.color: CelestinaTheme.border
            z: 5

            Text {
                id: opProgressTitle
                x: 12
                y: 9
                width: cancelOpButton.x - x - 12
                text: {
                    var label = controller.opCurrent.length > 0
                                ? controller.opCurrent : "Preparando…"
                    if (controller.opTotal > 1)
                        label += "  ·  " + (controller.opDone + 1)
                                 + " de " + controller.opTotal
                    return label
                }
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontLabel
                elide: Text.ElideMiddle
            }

            Text {
                id: opProgressDetail
                x: 12
                anchors.top: opProgressTitle.bottom
                anchors.topMargin: 3
                width: cancelOpButton.x - x - 12
                text: controller.opDetail
                visible: controller.opDetail.length > 0
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.interfaceTextScale)
                elide: Text.ElideRight
            }

            // Determinate bar over the top-level entry count.
            Rectangle {
                id: opProgressTrack
                x: 12
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 10
                width: cancelOpButton.x - x - 12
                height: 4
                radius: 2
                color: CelestinaTheme.controlFill

                Rectangle {
                    height: parent.height
                    radius: 2
                    color: CelestinaTheme.accent
                    width: controller.opTotal > 0
                           ? parent.width * Math.min(1, controller.opDone / controller.opTotal)
                           : 0
                    Behavior on width {
                        NumberAnimation { duration: CelestinaTheme.motionFast }
                    }
                }
            }

            Rectangle {
                id: cancelOpButton
                anchors.verticalCenter: parent.verticalCenter
                anchors.right: parent.right
                anchors.rightMargin: 12
                width: cancelOpLabel.width + 22
                height: 28
                radius: CelestinaTheme.radiusXs
                color: cancelOpMouse.containsMouse
                       ? CelestinaTheme.surfaceHover
                       : CelestinaTheme.controlFill
                border.width: 1
                border.color: CelestinaTheme.border

                Accessible.role: Accessible.Button
                Accessible.name: "Cancelar la operación"

                Text {
                    id: cancelOpLabel
                    anchors.centerIn: parent
                    text: "Cancelar"
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontMini
                    font.weight: CelestinaTheme.weightMedium
                }

                MouseArea {
                    id: cancelOpMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: controller.cancelOp()
                }
            }
        }

        Text {
            id: statusLine
            x: bottomControls.x + bottomControls.width + 14
            anchors.verticalCenter: bottomBar.verticalCenter
            width: Math.max(0, sizeButton.x - x - 12)
            // Only transient state here now (loading, a filtered count,
            // operation status, errors); item counts and the selected item's
            // details live in the sidebar info box. A lost watch is surfaced
            // truthfully — the list is a snapshot that may lag.
            text: controller.watchDegraded
                  ? "⚠ Vigilancia perdida · instantánea"
                  : controller.statusText
            color: controller.watchDegraded
                   ? CelestinaTheme.dangerText : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.interfaceTextScale)
            elide: Text.ElideRight
        }

        // Opens a submenu of independent size sliders (content vs sidebar,
        // icons vs text) — granular zoom, replacing the single slider.
        Button {
            id: sizeButton
            height: 34
            leftPadding: 18
            rightPadding: 18
            anchors.right: parent.right
            anchors.rightMargin: 16
            anchors.verticalCenter: bottomBar.verticalCenter
            text: "Tamaño"
            Accessible.name: "Ajustar tamaños"
            onClicked: sizePopup.opened ? sizePopup.close() : sizePopup.open()

            contentItem: Text {
                text: sizeButton.text
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.interfaceTextScale)
                font.weight: CelestinaTheme.weightMedium
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
            }

            background: GlassPill {
                backdrop: root.bottomView
                floating: root.bottomFloating
                fill: (sizeButton.hovered || sizePopup.opened)
                      ? CelestinaTheme.surfaceHover
                      : CelestinaTheme.controlFill
                border.width: sizeButton.activeFocus ? 1 : 0
                border.color: CelestinaTheme.focus
            }

            SizePopup {
                id: sizePopup
                // Float above the button, right-aligned to it.
                y: -height - 10
                x: sizeButton.width - width
                backdrop: mainPanel
                hostWindow: root.hostWindow
            }
        }
    }

    TopBar {
        id: topBar
        z: 10
        x: 12
        y: 12
        width: root.width - 24
        height: 52
        controller: controller
        activeView: mainPanel.viewMode === "grid" ? fileGrid : fileList
        hostWindow: root.hostWindow
        overlayParent: root.overlayParent
        pathMenu: pathMenu
        onViewFocusRequested: fileList.forceActiveFocus()
    }

    // ── Tab pills ────────────────────────────────────────────────────
    // A second floating row below the breadcrumb/search pills. Each tab is
    // an isolated pill — solid at rest, fading to glass as content scrolls
    // underneath, exactly like the pills above. Shown only with ≥2 tabs; the
    // strip scrolls (wheel / drag / bar) when tabs overflow.
    TabStrip {
        id: tabBar
        z: 10
        x: 12
        y: topBar.y + topBar.height + 8
        width: root.width - 24
        height: 34
        visible: root.hostWindow !== undefined && root.hostWindow.tabsModel.count >= 2
        controller: controller
        hostWindow: root.hostWindow
        topBar: topBar
        active: root.active
    }

    FolderSortMenu {
        id: sortMenu
        backdropSource: root
        controller: controller
    }

    EntryContextMenu {
        id: entryMenu
        backdropSource: root
        controller: controller
        panel: mainPanel
        namePrompt: namePrompt
        batchRename: batchRename
        iconPicker: iconPicker
        onNewTabRequested: function(path, foreground) { root.requestNewTab(path, foreground) }
    }

    // Context menu for the breadcrumb / path bar: act on the current path.
    PathMenu {
        id: pathMenu
        backdropSource: root
        controller: controller
        onNewTabRequested: function(path, foreground) { root.requestNewTab(path, foreground) }
    }

    FolderMenu {
        id: folderMenu
        backdropSource: root
        controller: controller
        panel: mainPanel
        namePrompt: namePrompt
        onNewTabRequested: function(path, foreground) { root.requestNewTab(path, foreground) }
    }

    // Los diálogos y overlays de esta vista, cada uno en su fichero.
    NamePromptDialog {
        id: namePrompt
        controller: controller
        owner: root
    }

    BatchRenameDialog {
        id: batchRename
        controller: controller
        owner: root
    }

    ConflictDialog {
        id: conflictDialog
        controller: controller
        owner: root
    }

    OpenWithDialog {
        id: openWithView
        controller: controller
        owner: root
    }

    PropertiesDialog {
        id: propertiesView
        controller: controller
        owner: root
    }

    IconPickerDialog {
        id: iconPicker
        controller: controller
        owner: root
        panel: mainPanel
    }

    QuickLookView {
        id: quickLookView
        controller: controller
        owner: root
        panel: mainPanel
    }








        // ── Recursive-search status bar ───────────────────────────────────
        // The hits themselves ride the same entryModel as the folder, so the
        // list/grid render and act on them identically (single-click selects,
        // double-click opens, keyboard, selection). This slim glass bar just
        // floats below the breadcrumb/tabs to show the query and offer Stop /
        // Close — the search results are the content view.
        SearchBar {
            id: searchBar
            z: 10
            x: 12
            width: root.width - 24
            height: 40
            y: (tabBar.visible ? tabBar.y + tabBar.height : topBar.y + topBar.height) + 8
            // Fades in place. Not a slide: these carry glass, and moving
            // a glass surface mid-animation samples the wrong region.
            visible: opacity > 0.01
            opacity: (controller.searchActive || controller.searchRunning) ? 1 : 0
            Behavior on opacity {
                NumberAnimation {
                    duration: CelestinaTheme.motionFast
                    easing.type: CelestinaTheme.easeStandard
                }
            }
            controller: controller
            backdrop: topBar.activeView
            textScale: root.hostWindow.interfaceTextScale
        }

        // ── Recientes location header ──────────────────────────────────
        // The same shape as the Trash header: a pill that says where you
        // are and how much is here, and the way back. Nothing else — this
        // list belongs to the desktop, and Siderita only reads it.
        RecentHeader {
            id: recentHeader
            z: 10
            x: 12
            width: root.width - 24
            height: 40
            y: (tabBar.visible ? tabBar.y + tabBar.height : topBar.y + topBar.height) + 8
            // Fades in place. Not a slide: these carry glass, and moving
            // a glass surface mid-animation samples the wrong region.
            visible: opacity > 0.01
            opacity: controller.recentActive ? 1 : 0
            Behavior on opacity {
                NumberAnimation {
                    duration: CelestinaTheme.motionFast
                    easing.type: CelestinaTheme.easeStandard
                }
            }
            controller: controller
            backdrop: topBar.activeView
            textScale: root.hostWindow.interfaceTextScale
        }

        // ── Trash location header ──────────────────────────────────────
        // Trashed items ride the same entryModel (like search), so the
        // content view renders them as list / grid / details with
        // thumbnails. This slim glass bar floats below the breadcrumb with
        // the bulk actions and the way back.
        TrashHeader {
            id: trashHeader
            z: 10
            x: 12
            width: root.width - 24
            height: 40
            y: (tabBar.visible ? tabBar.y + tabBar.height : topBar.y + topBar.height) + 8
            visible: controller.trashActive
            controller: controller
            backdrop: topBar.activeView
            textScale: root.hostWindow.interfaceTextScale
        }

        // ── Drag auto-scroll edges ─────────────────────────────────────
        // Two thin strips over the top and bottom of the view: while a drag
        // rests on one, the view scrolls, so a destination below the fold
        // does not mean dropping the entry somewhere else first. They sit
        // above the rows (a row would otherwise swallow the drag), so a
        // release on one lands in the *current* folder — the same thing an
        // empty-space drop does — and an entry dragged within its own
        // folder simply has nowhere to go.
        DragScrollEdge {
            id: topScrollEdge
            x: 8
            y: 14
            width: parent.width - 16
            view: topBar.activeView
            step: -18
            onExternalDrop: function(drop) {
                controller.dropUris(mainPanel.urlsToPaths(drop.urls),
                                    "", mainPanel.dropIsMove(drop))
                drop.accept()
            }
        }

        DragScrollEdge {
            id: bottomScrollEdge
            x: 8
            y: parent.height - 68 + 14 - height
            width: parent.width - 16
            view: topBar.activeView
            step: 18
            onExternalDrop: function(drop) {
                controller.dropUris(mainPanel.urlsToPaths(drop.urls),
                                    "", mainPanel.dropIsMove(drop))
                drop.accept()
            }
        }

        // ── Details-view column header ─────────────────────────────────
        // A floating glass strip aligned to the list's columns; each title
        // sorts by that field (a second click on the active one flips the
        // direction) and carries an ↑/↓ arrow.
        DetailsHeader {
            id: detailsHeader
            z: 10
            x: 8
            width: parent.width - 16
            height: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale) + 18
            y: (tabBar.visible ? tabBar.y + tabBar.height : topBar.y + topBar.height) + 8
            visible: fileList.detailsMode
            controller: controller
            view: fileList
            textScale: root.hostWindow.contentTextScale
        }
}
