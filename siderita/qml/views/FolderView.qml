import QtQuick
import QtQuick.Controls
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
    // Alias con nombre distinto para inyectar en hijos: una propiedad inyectada
    // no puede llamarse igual que el id que se le pasa (`x: x` se sombrea a sí
    // misma y queda undefined — la clase de bug del fix de clics, 9e19b6d).
    property alias viewTopBar: topBar
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

    FolderShortcuts {
        anchors.fill: parent
        viewActive: root.active
        canGoBackOrLeave: root.canGoBackOrLeave
        controller: tabController
        panel: mainPanel
        topBar: viewTopBar
        namePrompt: folderActions.namePrompt
        onGoBackOrLeaveRequested: root.goBackOrLeave()
    }

    CelestinaSurface {
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
        role: CelestinaSurface.Panel

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
            controller: tabController
            panel: mainPanel
            bottomView: root.bottomView
            bottomFloating: root.bottomFloating
            overlayParent: root.overlayParent
            sortMenu: folderActions.sortMenu
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
                folderActions.folderMenu.popup(root.overlayParent, point)
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
                color: CelestinaTheme.clear
                border.width: CelestinaTheme.borderFocus
                border.color: CelestinaTheme.accent
                radius: CelestinaTheme.radiusLg
                z: 40
            }
        }

        FolderListView {
            id: fileList
            x: 8
            y: 14
            width: parent.width - 16
            height: parent.height - 22
            controller: tabController
            entryModel: entryModel
            panel: mainPanel
            hostWindow: root.hostWindow
            ghost: root.ghost
            overlayParent: root.overlayParent
            contentTopMargin: 62 + (tabBar.visible ? tabBar.height + 8 : 0)
                              + (folderChrome.searchBar.visible
                                 ? folderChrome.searchBar.height + 8 : 0)
                              + (folderChrome.trashHeader.visible
                                 ? folderChrome.trashHeader.height + 8 : 0)
                              + (folderChrome.recentHeader.visible
                                 ? folderChrome.recentHeader.height + 8 : 0)
                              + (fileList.detailsMode
                                 ? folderChrome.detailsHeader.height + 8 : 0)
            onQuickLookRequested: root.quickLookOpen = true
            onNewTabRequested: function(path, foreground) {
                root.requestNewTab(path, foreground)
            }
            onContextMenuRequested: function(token, name, isDir, path, x, y) {
                folderActions.entryMenu.targetToken = token
                folderActions.entryMenu.targetName = name
                folderActions.entryMenu.targetDirectory = isDir
                folderActions.entryMenu.targetPath = path
                folderActions.entryMenu.popup(root.overlayParent, Qt.point(x, y))
            }
        }

        FolderGridView {
            id: fileGrid
            x: 8
            y: 14
            width: parent.width - 16
            height: parent.height - 22
            controller: tabController
            entryModel: entryModel
            panel: mainPanel
            hostWindow: root.hostWindow
            ghost: root.ghost
            overlayParent: root.overlayParent
            contentTopMargin: 62 + (tabBar.visible ? tabBar.height + 8 : 0)
                              + (folderChrome.searchBar.visible
                                 ? folderChrome.searchBar.height + 8 : 0)
                              + (folderChrome.trashHeader.visible
                                 ? folderChrome.trashHeader.height + 8 : 0)
                              + (folderChrome.recentHeader.visible
                                 ? folderChrome.recentHeader.height + 8 : 0)
            onQuickLookRequested: root.quickLookOpen = true
            onNewTabRequested: function(path, foreground) {
                root.requestNewTab(path, foreground)
            }
            onContextMenuRequested: function(token, name, isDir, path, x, y) {
                folderActions.entryMenu.targetToken = token
                folderActions.entryMenu.targetName = name
                folderActions.entryMenu.targetDirectory = isDir
                folderActions.entryMenu.targetPath = path
                folderActions.entryMenu.popup(root.overlayParent, Qt.point(x, y))
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
                radius: CelestinaTheme.radiusXs
                color: CelestinaTheme.selectionMarquee
                border.width: CelestinaTheme.borderHairline
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
                font.pixelSize: CelestinaTheme.fontHeaderCollapsed
                font.weight: CelestinaTheme.weightMedium
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: parent.searchEmpty
                      ? "Prueba con otra búsqueda."
                      : "No hay elementos que mostrar."
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
            }
        }

        FolderBottomStatus {
            anchors.fill: parent
            controller: tabController
            hostWindow: root.hostWindow
            panel: mainPanel
            bottomControls: bottomControls
            bottomView: root.bottomView
            bottomFloating: root.bottomFloating
        }
    }

    TopBar {
        id: topBar
        z: 10
        x: 12
        y: 12
        width: root.width - 24
        height: 52
        controller: tabController
        activeView: mainPanel.viewMode === "grid" ? fileGrid : fileList
        hostWindow: root.hostWindow
        overlayParent: root.overlayParent
        pathMenu: folderActions.pathMenu
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
        controller: tabController
        hostWindow: root.hostWindow
        topBar: viewTopBar
        active: root.active
    }

    FolderActions {
        id: folderActions
        anchors.fill: parent
        controller: tabController
        owner: root
        panel: mainPanel
        onNewTabRequested: function(path, foreground) {
            root.requestNewTab(path, foreground)
        }
    }

    FolderContentChrome {
        id: folderChrome
        anchors.fill: parent
        controller: tabController
        hostWindow: root.hostWindow
        panel: mainPanel
        topBar: viewTopBar
        tabBar: tabBar
        fileList: fileList
    }
}
