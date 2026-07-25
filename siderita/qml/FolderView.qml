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
    Connections {
        target: controller
        function onRowsReady(names, tokens, kinds, subtitles, paths, sections, sizes, dates) {
            entryModel.setRows(names, tokens, kinds, subtitles, paths, sections, sizes, dates)
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
        searchField.text = ""
        controller.applyQuery("")
        controller.closeSearch()
    }

    // Back undoes wherever you are, in the order you got there: a search
    // bows out first, then the Trash location, and only then does history
    // move. Trash is a place you can be but not a folder in history, so
    // without this the only way out was the sidebar.
    readonly property bool canGoBackOrLeave:
            !controller.loading
            && (searchField.text.length > 0 || controller.searchActive
                || controller.trashActive || controller.recentActive
                || controller.canGoBack)

    function goBackOrLeave() {
        if (controller.loading)
            return
        if (searchField.text.length > 0 || controller.searchActive)
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
        onActivated: pathPill.beginEditing()
    }

    Shortcut {
        sequence: "Ctrl+F"
        enabled: root.active
        onActivated: {
            searchField.forceActiveFocus()
            searchField.selectAll()
        }
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
                // A new folder (sidebar/place click, breadcrumb, back/up…)
                // returns the content box to its plain listing.
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
        RowLayout {
            id: bottomControls
            x: 16
            anchors.verticalCenter: bottomBar.verticalCenter
            spacing: 10

            GlassPill {
                id: hiddenToggle
                Layout.preferredWidth: hiddenLabel.implicitWidth + 22
                Layout.preferredHeight: 30
                backdrop: root.bottomView
                floating: root.bottomFloating
                fill: controller.showHidden
                      ? CelestinaTheme.badgeAccentFill
                      : hiddenMouse.containsMouse
                        ? CelestinaTheme.surfaceHover
                        : CelestinaTheme.controlFill

                Accessible.role: Accessible.Button
                Accessible.name: "Mostrar u ocultar elementos ocultos"

                Text {
                    id: hiddenLabel
                    anchors.centerIn: parent
                    text: "Ocultos"
                    color: controller.showHidden
                           ? CelestinaTheme.accent
                           : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontMini * root.hostWindow.interfaceTextScale)
                    font.weight: CelestinaTheme.weightMedium
                }

                MouseArea {
                    id: hiddenMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: controller.toggleHidden()
                }
            }

            Button {
                id: sortButton

                readonly property var labels: [
                    "Nombre", "Tamaño", "Fecha", "Tipo"
                ]

                Layout.preferredHeight: 34
                leftPadding: 16
                rightPadding: 16
                text: labels[controller.sortField]
                Accessible.name: "Ordenar por " + text
                onClicked: {
                    // Button is at the bottom now — open the menu upward.
                    // sortMenu.height can be 0 before the first open; fall back
                    // to an estimate for the four sort options.
                    const menuHeight = sortMenu.height > 0 ? sortMenu.height : 172
                    const point = sortButton.mapToItem(
                                    root.overlayParent, 0, -menuHeight - 6)
                    sortMenu.popup(root.overlayParent, point)
                }

                contentItem: Text {
                    text: "Orden: " + sortButton.text
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
                    fill: sortButton.hovered
                          ? CelestinaTheme.surfaceHover
                          : CelestinaTheme.controlFill
                    border.width: sortButton.activeFocus ? 1 : 0
                    border.color: CelestinaTheme.focus
                }
            }

            NavButton {
                id: sortDirectionButton
                Layout.alignment: Qt.AlignVCenter
                iconName: controller.sortAscending
                          ? "view-sort-ascending"
                          : "view-sort-descending"
                fallbackIcon: controller.sortAscending
                              ? "view-sort-ascending"
                              : "view-sort-descending"
                helpText: controller.sortAscending
                          ? "Orden ascendente"
                          : "Orden descendente"
                onClicked: controller.toggleSortDirection()
            }

            // Lista / Cuadrícula / Detalles: tres pastillas independientes,
            // no un segmentado dentro de una cápsula. Sigue siendo una sola
            // elección — lo dice el relleno del modo activo, no una caja
            // alrededor de los tres.
            Item {
                id: viewSeg
                Layout.preferredHeight: 30
                Layout.preferredWidth: viewSegRow.implicitWidth

                Row {
                    id: viewSegRow
                    anchors.centerIn: parent
                    spacing: 6

                    Repeater {
                        model: [
                            { mode: "list", label: "Lista" },
                            { mode: "grid", label: "Cuadrícula" },
                            { mode: "details", label: "Detalles" }
                        ]

                        delegate: GlassPill {
                            id: seg
                            required property var modelData
                            readonly property bool active: mainPanel.viewMode === modelData.mode
                            width: segLabel.implicitWidth + 22
                            height: 30
                            backdrop: root.bottomView
                            floating: root.bottomFloating
                            fill: seg.active ? CelestinaTheme.surfaceSelected
                                  : segMouse.containsMouse ? CelestinaTheme.surfaceHover
                                  : CelestinaTheme.controlFill

                            Accessible.role: Accessible.RadioButton
                            Accessible.name: "Vista " + seg.modelData.label
                            Accessible.checked: seg.active

                            Text {
                                id: segLabel
                                anchors.centerIn: parent
                                text: seg.modelData.label
                                color: seg.active ? CelestinaTheme.text : CelestinaTheme.textMuted
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.interfaceTextScale)
                                font.weight: seg.active ? CelestinaTheme.weightMedium
                                                        : CelestinaTheme.weightRegular
                            }

                            MouseArea {
                                id: segMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    mainPanel.viewMode = seg.modelData.mode
                                    mainPanel.persist()
                                }
                            }
                        }
                    }
                }
            }

            BusyIndicator {
                id: busy
                Layout.preferredWidth: 26
                Layout.preferredHeight: 26
                running: controller.loading
                visible: running
            }
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

            delegate: Item {
                id: row

                // Roles from the native SideritaEntryModel.
                required property int index
                required property string name
                required property string token
                required property string kind
                required property string subtitle
                required property string path
                required property bool isDirectory
                required property string sizeText
                required property string dateText

                readonly property bool selected: mainPanel.isSelected(token)
                // Hidden (dotfile) entries are dimmed so they read as a
                // distinct, secondary block.
                readonly property bool hidden: name.charAt(0) === "."
                // Ghosted while it sits on the clipboard as a cut (pending
                // move); an italic name tells it apart from a mere dotfile.
                readonly property bool cut: controller.cutPaths.indexOf(path) >= 0

                width: fileList.width
                height: mainPanel.listRowHeight
                opacity: cut ? 0.4 : hidden ? 0.5 : 1.0
                Accessible.role: Accessible.ListItem
                Accessible.name: name
                Accessible.selected: selected

                Rectangle {
                    anchors.fill: parent
                    anchors.leftMargin: 4
                    anchors.rightMargin: 4
                    radius: CelestinaTheme.radiusSm
                    color: row.selected
                           ? CelestinaTheme.surfaceSelected
                           : pointer.containsMouse
                             ? CelestinaTheme.surfaceHover
                             : "transparent"
                    border.width: row.selected ? 1 : 0
                    border.color: CelestinaTheme.borderStrong

                    Behavior on color {
                        ColorAnimation {
                            duration: CelestinaTheme.motionFast
                        }
                    }
                }

                // Drop onto this row when it is a folder → the drop lands
                // inside that folder. Accepts external file URLs and internal
                // entry drags (move a file/folder into this folder).
                DropArea {
                    id: rowDrop
                    anchors.fill: parent
                    enabled: row.isDirectory

                    onEntered: function(drag) {
                        if (!drag.hasUrls && !mainPanel.isEntryDrag(drag)) {
                            drag.accepted = false
                            return
                        }
                        springOpen.restart()
                    }
                    onExited: springOpen.stop()
                    onDropped: function(drop) {
                        springOpen.stop()
                        mainPanel.dropOnto(row.path, drop)
                        drop.accept()
                    }

                    // Spring-loaded: hold a drag over a folder and it opens,
                    // so a move into somewhere deep does not mean dropping
                    // it here first and picking it up again.
                    Timer {
                        id: springOpen
                        interval: CelestinaTheme.springDelay
                        onTriggered: {
                            if (rowDrop.containsDrag)
                                controller.openLocation(row.path)
                        }
                    }

                    Rectangle {
                        anchors.fill: parent
                        anchors.leftMargin: 4
                        anchors.rightMargin: 4
                        visible: parent.containsDrag
                        color: "transparent"
                        radius: CelestinaTheme.radiusSm
                        border.width: 2
                        border.color: CelestinaTheme.accent
                    }
                }

                Rectangle {
                    id: kindGlyph
                    x: 14
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.round(CelestinaTheme.glyphTile * root.hostWindow.contentIconScale)
                    height: Math.round(CelestinaTheme.glyphTile * root.hostWindow.contentIconScale)
                    radius: CelestinaTheme.radiusSm
                    color: row.kind === "directory"
                           ? CelestinaTheme.glyphDirectory
                           : row.kind === "symlink"
                             ? CelestinaTheme.glyphSymlink
                             : CelestinaTheme.glyphFile
                    clip: true

                    readonly property string media: row.kind === "directory"
                                                    ? "" : mainPanel.mediaKind(row.name)

                    IconImage {
                        anchors.centerIn: parent
                        visible: !thumb.ready
                        width: Math.round(CelestinaTheme.iconMd * root.hostWindow.contentIconScale)
                        height: Math.round(CelestinaTheme.iconMd * root.hostWindow.contentIconScale)
                        name: mainPanel.mediaIconName(row.kind, kindGlyph.media, row.path)
                        // El icono elegido a mano suele ser simbólico, y los
                        // simbólicos sólo se publican a 16 px: sin pedir el
                        // tamaño explícito se dibujan diminutos dentro de una
                        // celda hecha para una carpeta de 54.
                        sourceSize: Qt.size(width, height)
                        source: CelestinaTheme.fallbackIcon(
                                    row.kind === "directory"
                                    ? "folder"
                                    : row.kind === "symlink"
                                      ? "symlink"
                                      : "file")
                        // No color tint: let the icon theme (Qogir) render
                        // folders and mimetypes in their own colours. A tint
                        // would flatten them to a solid silhouette.
                    }

                    // The cached image / video-frame / cover the "thumb"
                    // provider returns, covering the tile once decoded; the
                    // themed glyph shows until then (or forever, for media the
                    // cache has no thumbnail of).
                    Image {
                        id: thumb
                        anchors.fill: parent
                        anchors.margins: 1
                        readonly property bool ready: kindGlyph.media !== ""
                                                      && status === Image.Ready
                        // Fades up as it decodes, so a folder of photos
                        // fills in instead of flickering glyph→picture.
                        visible: opacity > 0
                        opacity: ready ? 1 : 0
                        Behavior on opacity {
                            NumberAnimation { duration: CelestinaTheme.motionNormal }
                        }
                        source: kindGlyph.media !== ""
                                ? "image://thumb/" + encodeURIComponent(row.path) : ""
                        sourceSize.width: 256
                        sourceSize.height: 256
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        cache: true
                        smooth: true
                    }

                    FavoriteBadge {
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        anchors.margins: 1
                        diameter: Math.round(13 * root.hostWindow.contentIconScale)
                        starred: mainPanel.isFavorite(row.path)
                    }

                    // A small play badge marks a video's frame apart from a
                    // still image.
                    Rectangle {
                        visible: thumb.ready && kindGlyph.media === "video"
                        anchors.centerIn: parent
                        width: Math.round(parent.width * 0.42)
                        height: width
                        radius: width / 2
                        color: Qt.rgba(0, 0, 0, 0.45)
                        Text {
                            anchors.centerIn: parent
                            text: "▶"
                            color: "white"
                            font.pixelSize: Math.round(parent.width * 0.5)
                        }
                    }
                }

                // List / search body: name over the combined subtitle.
                Column {
                    id: rowText
                    visible: !fileList.detailsMode
                    x: kindGlyph.x + kindGlyph.width + 12
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - x - 24
                    spacing: 1

                    Text {
                        width: parent.width
                        text: row.name
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.contentTextScale)
                        font.weight: CelestinaTheme.weightMedium
                        font.italic: row.cut
                        elide: Text.ElideMiddle
                    }

                    Text {
                        width: parent.width
                        text: row.subtitle
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale)
                        elide: Text.ElideRight
                    }
                }

                // Details body: name (fills) · size · date · type, aligned to
                // the header's columns.
                RowLayout {
                    visible: fileList.detailsMode
                    x: fileList.detailsNameX
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - x - 16
                    spacing: 12

                    Text {
                        Layout.fillWidth: true
                        text: row.name
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.contentTextScale)
                        font.weight: CelestinaTheme.weightMedium
                        font.italic: row.cut
                        elide: Text.ElideMiddle
                    }
                    Text {
                        Layout.preferredWidth: fileList.colSizeW
                        horizontalAlignment: Text.AlignRight
                        text: row.sizeText
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale)
                        elide: Text.ElideRight
                    }
                    Text {
                        Layout.preferredWidth: fileList.colDateW
                        text: row.dateText
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale)
                        elide: Text.ElideRight
                    }
                    Text {
                        Layout.preferredWidth: fileList.colTypeW
                        text: row.kind === "directory" ? "Carpeta"
                              : row.kind === "symlink" ? "Enlace" : "Archivo"
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale)
                        elide: Text.ElideRight
                    }
                }

                MouseArea {
                    id: pointer
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                    hoverEnabled: true

                    onClicked: function(mouse) {
                        if (mouse.button === Qt.MiddleButton) {
                            // Middle-click a folder → new background tab.
                            if (row.isDirectory)
                                root.requestNewTab(
                                    row.path, false)
                            return
                        }
                        fileList.forceActiveFocus()
                        fileList.currentIndex = row.index
                        if (mouse.button === Qt.RightButton) {
                            if (!mainPanel.isSelected(row.token))
                                mainPanel.selectOnly(row.token)
                        } else if (mouse.modifiers & Qt.ControlModifier) {
                            mainPanel.toggleSelection(row.token)
                        } else if (mouse.modifiers & Qt.ShiftModifier) {
                            mainPanel.selectRange(row.index)
                        } else {
                            mainPanel.selectOnly(row.token)
                        }
                        controller.selectToken(row.token)
                        if (mouse.button === Qt.RightButton) {
                            const point = row.mapToItem(
                                            root.overlayParent,
                                            mouse.x, mouse.y)
                            entryMenu.targetToken = row.token
                            entryMenu.targetName = row.name
                            entryMenu.targetDirectory =
                                    row.isDirectory
                            entryMenu.targetPath = row.path
                            entryMenu.popup(root.overlayParent, point)
                        }
                    }

                    onDoubleClicked: function(mouse) {
                        if (mouse.button === Qt.LeftButton)
                            controller.activateToken(row.token)
                    }
                }

                DragHandler {
                    id: rowDrag
                    target: null
                    dragThreshold: 8
                    // Any entry is draggable (a file to move onto a folder, a
                    // folder to move or to bookmark on the sidebar).
                    enabled: true
                    onActiveChanged: {
                        if (active)
                            mainPanel.startEntryDrag(
                                row.path, row.name, row.isDirectory, kindGlyph, rowDrag)
                        else {
                            root.ghost.Drag.drop()
                            root.ghost.Drag.active = false
                        }
                    }
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

            delegate: Item {
                id: cell

                // Roles from the native SideritaEntryModel.
                required property int index
                required property string name
                required property string token
                required property string kind
                required property string subtitle
                required property string path
                required property bool isDirectory

                readonly property bool selected: mainPanel.isSelected(token)
                readonly property bool hidden: name.charAt(0) === "."
                // Ghosted while cut (pending move); italic name distinguishes
                // it from a dimmed dotfile.
                readonly property bool cut: controller.cutPaths.indexOf(path) >= 0

                width: fileGrid.cellWidth
                height: fileGrid.cellHeight
                opacity: cut ? 0.4 : hidden ? 0.5 : 1.0
                Accessible.role: Accessible.ListItem
                Accessible.name: name
                Accessible.selected: selected

                // The selection square keeps its natural size and centres in
                // the (stretched-to-fill) cell, rather than ballooning to the
                // full column width.
                Rectangle {
                    anchors.centerIn: parent
                    width: mainPanel.gridCellWidth - 10
                    height: parent.height - 10
                    radius: CelestinaTheme.radiusSm
                    color: cell.selected
                           ? CelestinaTheme.surfaceSelected
                           : cellMouse.containsMouse
                             ? CelestinaTheme.surfaceHover
                             : "transparent"
                    border.width: cell.selected ? 1 : 0
                    border.color: CelestinaTheme.borderStrong

                    Behavior on color {
                        ColorAnimation {
                            duration: CelestinaTheme.motionFast
                        }
                    }
                }

                // Drop onto this cell when it is a folder (external file URLs
                // or an internal entry drag).
                DropArea {
                    id: cellDrop
                    anchors.fill: parent
                    anchors.margins: 5
                    enabled: cell.isDirectory

                    onEntered: function(drag) {
                        if (!drag.hasUrls && !mainPanel.isEntryDrag(drag)) {
                            drag.accepted = false
                            return
                        }
                        cellSpringOpen.restart()
                    }
                    onExited: cellSpringOpen.stop()
                    onDropped: function(drop) {
                        cellSpringOpen.stop()
                        mainPanel.dropOnto(cell.path, drop)
                        drop.accept()
                    }

                    // Spring-loaded, like the list rows.
                    Timer {
                        id: cellSpringOpen
                        interval: CelestinaTheme.springDelay
                        onTriggered: {
                            if (cellDrop.containsDrag)
                                controller.openLocation(cell.path)
                        }
                    }

                    Rectangle {
                        anchors.centerIn: parent
                        width: mainPanel.gridCellWidth - 10
                        height: parent.height - 10
                        visible: parent.containsDrag
                        color: "transparent"
                        radius: CelestinaTheme.radiusSm
                        border.width: 2
                        border.color: CelestinaTheme.accent
                    }
                }

                Column {
                    anchors.centerIn: parent
                    spacing: 8

                    Rectangle {
                        id: cellGlyph
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Math.round(72 * root.hostWindow.contentIconScale)
                        height: Math.round(72 * root.hostWindow.contentIconScale)
                        radius: CelestinaTheme.radiusSm
                        clip: true
                        color: cell.kind === "directory"
                               ? CelestinaTheme.glyphDirectory
                               : cell.kind === "symlink"
                                 ? CelestinaTheme.glyphSymlink
                                 : CelestinaTheme.glyphFile

                        readonly property string media: cell.kind === "directory"
                                                        ? "" : mainPanel.mediaKind(cell.name)

                        IconImage {
                            anchors.centerIn: parent
                            visible: !cellThumb.ready
                            width: Math.round(54 * root.hostWindow.contentIconScale)
                            height: Math.round(54 * root.hostWindow.contentIconScale)
                            name: mainPanel.mediaIconName(cell.kind, cellGlyph.media, cell.path)
                            sourceSize: Qt.size(width, height)
                            source: CelestinaTheme.fallbackIcon(
                                        cell.kind === "directory"
                                        ? "folder"
                                        : cell.kind === "symlink"
                                          ? "symlink"
                                          : "file")
                            // No color tint — see the list delegate above.
                        }

                        Image {
                            id: cellThumb
                            anchors.fill: parent
                            anchors.margins: 1
                            readonly property bool ready: cellGlyph.media !== ""
                                                          && status === Image.Ready
                            visible: opacity > 0
                            opacity: ready ? 1 : 0
                            Behavior on opacity {
                                NumberAnimation { duration: CelestinaTheme.motionNormal }
                            }
                            source: cellGlyph.media !== ""
                                    ? "image://thumb/" + encodeURIComponent(cell.path) : ""
                            sourceSize.width: 256
                            sourceSize.height: 256
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                            cache: true
                            smooth: true
                        }

                        FavoriteBadge {
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            anchors.margins: 2
                            iconScale: root.hostWindow.contentIconScale
                            starred: mainPanel.isFavorite(cell.path)
                        }

                        // Play badge on a video frame.
                        Rectangle {
                            visible: cellThumb.ready && cellGlyph.media === "video"
                            anchors.centerIn: parent
                            width: Math.round(parent.width * 0.4)
                            height: width
                            radius: width / 2
                            color: Qt.rgba(0, 0, 0, 0.45)
                            Text {
                                anchors.centerIn: parent
                                text: "▶"
                                color: "white"
                                font.pixelSize: Math.round(parent.width * 0.5)
                            }
                        }
                    }

                    Text {
                        width: mainPanel.gridCellWidth - 22
                        horizontalAlignment: Text.AlignHCenter
                        text: cell.name
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale)
                        font.italic: cell.cut
                        elide: Text.ElideRight
                        maximumLineCount: 2
                        wrapMode: Text.Wrap
                    }
                }

                MouseArea {
                    id: cellMouse
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                    hoverEnabled: true

                    onClicked: function(mouse) {
                        if (mouse.button === Qt.MiddleButton) {
                            // Middle-click a folder → new background tab.
                            if (cell.isDirectory)
                                root.requestNewTab(
                                    cell.path, false)
                            return
                        }
                        fileGrid.forceActiveFocus()
                        fileGrid.currentIndex = cell.index
                        if (mouse.button === Qt.RightButton) {
                            if (!mainPanel.isSelected(cell.token))
                                mainPanel.selectOnly(cell.token)
                        } else if (mouse.modifiers & Qt.ControlModifier) {
                            mainPanel.toggleSelection(cell.token)
                        } else if (mouse.modifiers & Qt.ShiftModifier) {
                            mainPanel.selectRange(cell.index)
                        } else {
                            mainPanel.selectOnly(cell.token)
                        }
                        controller.selectToken(cell.token)
                        if (mouse.button === Qt.RightButton) {
                            const point = cell.mapToItem(
                                            root.overlayParent,
                                            mouse.x, mouse.y)
                            entryMenu.targetToken = cell.token
                            entryMenu.targetName = cell.name
                            entryMenu.targetDirectory =
                                    cell.isDirectory
                            entryMenu.targetPath = cell.path
                            entryMenu.popup(root.overlayParent, point)
                        }
                    }

                    onDoubleClicked: function(mouse) {
                        if (mouse.button === Qt.LeftButton)
                            controller.activateToken(cell.token)
                    }
                }

                DragHandler {
                    id: cellDrag
                    target: null
                    dragThreshold: 8
                    enabled: true
                    onActiveChanged: {
                        if (active)
                            mainPanel.startEntryDrag(
                                cell.path, cell.name, cell.isDirectory, cellGlyph, cellDrag)
                        else {
                            root.ghost.Drag.drop()
                            root.ghost.Drag.active = false
                        }
                    }
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
            y: 14
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
            y: errorBanner.visible ? errorBanner.y + errorBanner.height + 8 : 14
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
            y: opErrorBanner.visible
               ? opErrorBanner.y + opErrorBanner.height + 8
               : (errorBanner.visible ? errorBanner.y + errorBanner.height + 8 : 14)
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

            Popup {
                id: sizePopup
                // Float above the button, right-aligned to it.
                y: -height - 10
                x: sizeButton.width - width
                padding: 16
                // Non-modal so the content still scrolls (to watch items
                // resize) while sizes are adjusted; a click outside still
                // closes it via CloseOnPressOutside.
                modal: false
                dim: false
                focus: true
                closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

                // Frosted like the menus and dialogs — glass is the suite's
                // surface language. Samples the view behind it.
                background: GlassCard {
                    backdropSource: mainPanel
                    cornerRadius: CelestinaTheme.radiusLg
                }

                contentItem: Column {
                    spacing: 6

                    Text {
                        text: "ICONOS"
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontMini
                        font.letterSpacing: 1.4
                        font.weight: CelestinaTheme.weightDemiBold
                    }
                    SizeRow {
                        label: "Contenido"
                        value: root.hostWindow.contentIconScale
                        maxValue: 3.0
                        onMoved: function(v) {
                            root.hostWindow.contentIconScale = v
                            root.hostWindow.persistSizing()
                        }
                    }
                    SizeRow {
                        label: "Interfaz"
                        value: root.hostWindow.interfaceIconScale
                        onMoved: function(v) {
                            root.hostWindow.interfaceIconScale = v
                            root.hostWindow.persistSizing()
                        }
                    }
                    SizeRow {
                        label: "Barra lateral"
                        value: root.hostWindow.sidebarIconScale
                        onMoved: function(v) {
                            root.hostWindow.sidebarIconScale = v
                            root.hostWindow.persistSizing()
                        }
                    }

                    Item { width: 1; height: 4 }

                    Text {
                        text: "TEXTO"
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontMini
                        font.letterSpacing: 1.4
                        font.weight: CelestinaTheme.weightDemiBold
                    }
                    SizeRow {
                        label: "Contenido"
                        value: root.hostWindow.contentTextScale
                        onMoved: function(v) {
                            root.hostWindow.contentTextScale = v
                            root.hostWindow.persistSizing()
                        }
                    }
                    SizeRow {
                        label: "Interfaz"
                        value: root.hostWindow.interfaceTextScale
                        onMoved: function(v) {
                            root.hostWindow.interfaceTextScale = v
                            root.hostWindow.persistSizing()
                        }
                    }
                    SizeRow {
                        label: "Barra lateral"
                        value: root.hostWindow.sidebarTextScale
                        onMoved: function(v) {
                            root.hostWindow.sidebarTextScale = v
                            root.hostWindow.persistSizing()
                        }
                    }
                }
            }
        }
    }

    Item {
        id: topBar
        z: 10
        x: 12
        y: 12
        width: root.width - 24
        height: 52

        // Scroll offset of the active view (0 at the very top).
        readonly property real scrollY: mainPanel.viewMode === "grid"
                                        ? fileGrid.contentY + fileGrid.topMargin
                                        : fileList.contentY + fileList.topMargin
        // Once scrolled, each independent pill fades to glass in place.
        readonly property bool floating: scrollY > 6
        readonly property Item activeView: mainPanel.viewMode === "grid"
                                           ? fileGrid : fileList

        // Pulsed each time the pills refresh their capture, so the floating
        // tab pills below refresh their glass in the same beat.
        signal glassTick()

        function refreshGlass() {
            pathGlass.refreshBackdrop()
            searchGlass.refreshBackdrop()
            topBar.glassTick()
        }

        onFloatingChanged: if (floating) Qt.callLater(topBar.refreshGlass)

        // Refresh the blur as content scrolls under the pills; work stops
        // when scrolling stops (no continuous work at rest).
        Connections {
            target: topBar.activeView
            function onContentYChanged() {
                if (topBar.floating)
                    topBar.refreshGlass()
            }
        }

        Rectangle {
            id: pathPill

            property bool editing: false

            function beginEditing() {
                editing = true
                locationField.text = controller.currentPath
                locationField.forceActiveFocus()
                locationField.selectAll()
            }

            function cancelEditing() {
                editing = false
                fileList.forceActiveFocus()
            }

            function pathSegments(p) {
                if (!p || p.length === 0)
                    return []
                const parts = p.split("/")
                const segs = []
                let acc = ""
                for (let idx = 0; idx < parts.length; idx++) {
                    const part = parts[idx]
                    if (part.length === 0) {
                        if (idx === 0)
                            segs.push({ name: "/", path: "/" })
                        continue
                    }
                    acc = acc + "/" + part
                    segs.push({ name: part, path: acc })
                }
                return segs
            }

            x: 14
            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(180, searchField.x - x - 12)
            height: CelestinaTheme.controlHeight
            radius: CelestinaTheme.radiusSm
            clip: true
            color: CelestinaTheme.inputFill
            border.width: 1
            border.color: topBar.floating ? "transparent" : CelestinaTheme.inputBorder

            GlassSurface {
                id: pathGlass
                anchors.fill: parent
                backdropSource: topBar.activeView
                captureEnabled: topBar.floating
                cornerRadius: parent.radius
                opacity: topBar.floating ? 1 : 0
                Behavior on opacity {
                    NumberAnimation { duration: CelestinaTheme.motionNormal }
                }
            }

            MouseArea {
                id: pathMouse
                anchors.fill: parent
                visible: !pathPill.editing
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                cursorShape: Qt.IBeamCursor
                Accessible.name: "Editar ubicación"
                onClicked: function(mouse) {
                    if (mouse.button === Qt.RightButton) {
                        const point = pathMouse.mapToItem(
                                        root.overlayParent, mouse.x, mouse.y)
                        pathMenu.popup(root.overlayParent, point)
                    } else {
                        pathPill.beginEditing()
                    }
                }
            }

            Row {
                id: crumbRow
                anchors.right: parent.right
                anchors.rightMargin: 13
                anchors.verticalCenter: parent.verticalCenter
                visible: !pathPill.editing
                spacing: 3

                Repeater {
                    id: crumbRepeater
                    model: pathPill.pathSegments(controller.currentPath)

                    delegate: Row {
                        id: crumb

                        required property var modelData
                        required property int index

                        spacing: 3
                        anchors.verticalCenter: parent.verticalCenter

                        Text {
                            visible: crumb.index > 0
                            anchors.verticalCenter: parent.verticalCenter
                            text: "›"
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontLabel * root.hostWindow.interfaceTextScale)
                        }

                        Rectangle {
                            anchors.verticalCenter: parent.verticalCenter
                            width: crumbText.implicitWidth + 12
                            height: 24
                            radius: CelestinaTheme.radiusXs
                            color: crumbMouse.containsMouse
                                   ? CelestinaTheme.surfaceHover
                                   : "transparent"

                            Text {
                                id: crumbText
                                anchors.centerIn: parent
                                text: crumb.modelData.name
                                color: crumb.index === crumbRepeater.count - 1
                                       ? CelestinaTheme.text
                                       : CelestinaTheme.textMuted
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: Math.round(CelestinaTheme.fontLabel * root.hostWindow.interfaceTextScale)
                            }

                            MouseArea {
                                id: crumbMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: controller.openLocation(
                                               crumb.modelData.path)
                            }
                        }
                    }
                }
            }

            TextField {
                id: locationField

                anchors.fill: parent
                visible: pathPill.editing
                leftPadding: 13
                rightPadding: 13
                color: CelestinaTheme.text
                selectionColor: CelestinaTheme.accentStrong
                selectedTextColor: CelestinaTheme.text
                font.family: CelestinaTheme.monoFamily
                font.pixelSize: Math.round(CelestinaTheme.fontLabel * root.hostWindow.interfaceTextScale)
                background: null
                Accessible.name: "Ubicación"

                onActiveFocusChanged: {
                    if (!activeFocus && pathPill.editing)
                        pathPill.editing = false
                }

                onAccepted: {
                    const location = text
                    pathPill.editing = false
                    controller.openLocation(location)
                    fileList.forceActiveFocus()
                }

                Keys.onPressed: function(event) {
                    if (event.key === Qt.Key_Escape) {
                        pathPill.cancelEditing()
                        event.accepted = true
                    }
                }
            }

            Connections {
                target: controller

                function onCurrentPathChanged() {
                    if (pathPill.editing)
                        pathPill.editing = false
                }
            }
        }

        TextField {
            id: searchField
            // Flexes with the interface text scale — the field grows and the
            // breadcrumb (which fills the rest) yields space — so a larger
            // search text is never clipped.
            width: Math.round(Math.min(topBar.width * 0.42,
                                       Math.max(190, 180 * root.hostWindow.interfaceTextScale)))
            height: CelestinaTheme.controlHeight
            x: topBar.width - width - 14
            anchors.verticalCenter: parent.verticalCenter
            placeholderText: "Buscar aquí y en subcarpetas"
            color: CelestinaTheme.text
            placeholderTextColor: CelestinaTheme.textMuted
            selectionColor: CelestinaTheme.accentStrong
            selectedTextColor: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.interfaceTextScale)
            leftPadding: 13
            rightPadding: 13
            // Typing always searches — a recursive walk grouped into "in
            // this folder" and "in subfolders"; clearing it exits search.
            onTextEdited: searchDebounce.restart()
            onAccepted: if (text.trim().length > 0)
                            controller.searchRecursive(text)

            background: Item {
                Rectangle {
                    anchors.fill: parent
                    radius: CelestinaTheme.radiusSm
                    color: searchField.activeFocus
                           ? CelestinaTheme.inputFillFocus
                           : CelestinaTheme.inputFill
                    border.width: 1
                    border.color: topBar.floating
                                  ? "transparent"
                                  : searchField.activeFocus
                                    ? CelestinaTheme.focus
                                    : CelestinaTheme.inputBorder
                }
                GlassSurface {
                    id: searchGlass
                    anchors.fill: parent
                    backdropSource: topBar.activeView
                    captureEnabled: topBar.floating
                    cornerRadius: CelestinaTheme.radiusSm
                    opacity: topBar.floating ? 1 : 0
                    Behavior on opacity {
                        NumberAnimation { duration: CelestinaTheme.motionNormal }
                    }
                }
            }
        }

        Timer {
            id: searchDebounce
            interval: 220
            repeat: false
            onTriggered: {
                if (searchField.text.trim().length > 0)
                    controller.searchRecursive(searchField.text)
                else
                    controller.closeSearch()
            }
        }

    }

    // ── Tab pills ────────────────────────────────────────────────────
    // A second floating row below the breadcrumb/search pills. Each tab is
    // an isolated pill — solid at rest, fading to glass as content scrolls
    // underneath, exactly like the pills above. Shown only with ≥2 tabs; the
    // strip scrolls (wheel / drag / bar) when tabs overflow.
    Item {
        id: tabBar
        z: 10
        x: 12
        y: topBar.y + topBar.height + 8
        width: root.width - 24
        height: 34
        visible: root.hostWindow !== undefined && root.hostWindow.tabsModel.count >= 2

        ListView {
            id: tabList
            anchors.left: parent.left
            anchors.right: newTabButton.left
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            height: parent.height
            orientation: ListView.Horizontal
            spacing: 8
            clip: true
            model: root.hostWindow ? root.hostWindow.tabsModel : null
            currentIndex: root.hostWindow ? root.hostWindow.currentTabIndex : 0
            boundsBehavior: Flickable.StopAtBounds
            flickableDirection: Flickable.HorizontalFlick

            Connections {
                target: root.hostWindow
                function onCurrentTabIndexChanged() {
                    tabList.positionViewAtIndex(root.hostWindow.currentTabIndex,
                                                ListView.Contain)
                }
            }
            // Chips move relative to the backdrop when the strip scrolls.
            onContentXChanged: if (topBar.floating) topBar.glassTick()

            delegate: Item {
                id: chip

                required property int index
                required property string title

                readonly property bool activeTab: root.hostWindow
                        && index === root.hostWindow.currentTabIndex
                readonly property int tabCount: root.hostWindow
                        ? root.hostWindow.tabsModel.count : 1

                // Tabs flex to share the strip's width (clamped), so they
                // shrink to fit as more open instead of overflowing off-edge;
                // only past the minimum does the strip start to scroll.
                width: Math.max(96, Math.min(200,
                        (tabList.width - (chip.tabCount - 1) * tabList.spacing)
                        / chip.tabCount))
                height: tabList.height

                // Solid pill at rest.
                Rectangle {
                    id: chipFill
                    anchors.fill: parent
                    radius: CelestinaTheme.radiusSm
                    color: chip.activeTab ? CelestinaTheme.surfaceSelected
                                          : chipMouse.containsMouse ? CelestinaTheme.surfaceHover
                                          : CelestinaTheme.inputFill
                    border.width: chip.activeTab ? 1 : (topBar.floating ? 0 : 1)
                    border.color: chip.activeTab ? CelestinaTheme.borderStrong
                                                 : CelestinaTheme.inputBorder

                    Behavior on color {
                        ColorAnimation { duration: CelestinaTheme.motionFast }
                    }
                }

                // …fading to glass when content scrolls under the strip.
                GlassSurface {
                    id: chipGlass
                    anchors.fill: parent
                    backdropSource: topBar.activeView
                    captureEnabled: root.active && topBar.floating
                    cornerRadius: CelestinaTheme.radiusSm
                    opacity: (root.active && topBar.floating) ? 1 : 0
                    Behavior on opacity {
                        NumberAnimation { duration: CelestinaTheme.motionNormal }
                    }
                    Connections {
                        target: topBar
                        function onGlassTick() { chipGlass.refreshBackdrop() }
                    }
                    Component.onCompleted: if (root.active && topBar.floating)
                                               Qt.callLater(chipGlass.refreshBackdrop)
                }

                IconImage {
                    id: chipIcon
                    x: 12
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.round(CelestinaTheme.iconSm * root.hostWindow.interfaceIconScale)
                    height: CelestinaTheme.iconSm
                    name: "folder"
                    source: CelestinaTheme.fallbackIcon("folder")
                    color: chip.activeTab ? CelestinaTheme.accent
                                          : CelestinaTheme.textMuted
                }

                Text {
                    x: chipIcon.x + chipIcon.width + 8
                    anchors.verticalCenter: parent.verticalCenter
                    width: closeButton.x - x - 6
                    text: chip.title.length > 0 ? chip.title : "…"
                    color: chip.activeTab ? CelestinaTheme.text
                                          : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontLabel * root.hostWindow.interfaceTextScale)
                    font.weight: chip.activeTab ? CelestinaTheme.weightMedium
                                                : CelestinaTheme.weightRegular
                    elide: Text.ElideRight
                }

                Rectangle {
                    id: closeButton
                    anchors.verticalCenter: parent.verticalCenter
                    x: parent.width - width - 8
                    width: 20
                    height: 20
                    radius: CelestinaTheme.radiusXs
                    color: closeMouse.containsMouse
                           ? CelestinaTheme.surfaceHover : "transparent"

                    Text {
                        anchors.centerIn: parent
                        text: "×"
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.interfaceTextScale)
                    }

                    MouseArea {
                        id: closeMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.hostWindow.closeTab(chip.index)
                    }
                }

                MouseArea {
                    id: chipMouse
                    anchors.fill: parent
                    anchors.rightMargin: 28   // leave the × its own handler
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: function(mouse) {
                        if (mouse.button === Qt.MiddleButton)
                            root.hostWindow.closeTab(chip.index)
                        else
                            root.hostWindow.selectTab(chip.index)
                    }
                }
            }

            ScrollBar.horizontal: ScrollBar {
                policy: ScrollBar.AsNeeded
                height: 4
            }
        }

        NavButton {
            id: newTabButton
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            iconName: "tab-new"
            fallbackIcon: "folder"
            helpText: "Nueva pestaña (Ctrl+T)"
            onClicked: root.hostWindow.openTab(controller.currentPath, true)
        }
    }

    GlassContextMenu {
        id: sortMenu
        backdropSource: root

        GlassMenuItem {
            text: "Nombre"
            current: controller.sortField === 0
            onTriggered: controller.changeSortField(0)
        }

        GlassMenuItem {
            text: "Tamaño"
            current: controller.sortField === 1
            onTriggered: controller.changeSortField(1)
        }

        GlassMenuItem {
            text: "Fecha de modificación"
            current: controller.sortField === 2
            onTriggered: controller.changeSortField(2)
        }

        GlassMenuItem {
            text: "Tipo"
            current: controller.sortField === 3
            onTriggered: controller.changeSortField(3)
        }
    }

    GlassContextMenu {
        id: entryMenu
        backdropSource: root

        property string targetToken: ""
        property string targetName: ""
        property bool targetDirectory: false
        property string targetPath: ""
        // How many entries the batch-capable verbs (copy/cut/trash) will act
        // on: the whole selection when the right-clicked entry is part of a
        // multi-selection, otherwise just this one.
        readonly property int actingCount:
                mainPanel.actingCount(targetToken)
        readonly property bool multi: actingCount > 1

        // ── Trash-only actions ──
        GlassMenuItem {
            text: "Restaurar"
            // Fades in place. Not a slide: these carry glass, and moving
            // a glass surface mid-animation samples the wrong region.
            visible: opacity > 0.01
            opacity: controller.trashActive ? 1 : 0
            Behavior on opacity {
                NumberAnimation {
                    duration: CelestinaTheme.motionFast
                    easing.type: CelestinaTheme.easeStandard
                }
            }
            height: visible ? implicitHeight : 0
            icon.name: "edit-undo"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: controller.restoreTrash(
                             controller.indexForToken(entryMenu.targetToken))
        }
        GlassMenuItem {
            text: "Eliminar permanentemente"
            visible: controller.trashActive
            height: visible ? implicitHeight : 0
            icon.name: "edit-delete"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: controller.purgeTrash(
                             controller.indexForToken(entryMenu.targetToken))
        }

        GlassMenuItem {
            text: entryMenu.targetDirectory ? "Abrir carpeta" : "Abrir"
            visible: !entryMenu.multi && !controller.trashActive
            height: visible ? implicitHeight : 0
            icon.name: entryMenu.targetDirectory ? "folder-open" : "text-x-generic"
            icon.source: CelestinaTheme.fallbackIcon(
                             entryMenu.targetDirectory ? "folder" : "file")
            onTriggered: controller.activateToken(entryMenu.targetToken)
        }

        GlassMenuItem {
            text: "Abrir con…"
            visible: !entryMenu.targetDirectory && !entryMenu.multi && !controller.trashActive
            height: visible ? implicitHeight : 0
            icon.name: "system-run"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: controller.openWith(entryMenu.targetPath)
        }

        GlassMenuItem {
            text: "Abrir en pestaña nueva"
            visible: entryMenu.targetDirectory && !entryMenu.multi && !controller.trashActive
            height: visible ? implicitHeight : 0
            icon.name: "tab-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: root.requestNewTab(entryMenu.targetPath, true)
        }

        GlassMenuItem {
            text: "Añadir a marcadores"
            visible: entryMenu.targetDirectory && !entryMenu.multi && !controller.trashActive
            height: visible ? implicitHeight : 0
            icon.name: "bookmark-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: controller.addBookmark(entryMenu.targetPath)
        }

        GlassMenuItem {
            text: mainPanel.isFavorite(entryMenu.targetPath)
                  ? "Quitar de favoritos" : "Añadir a favoritos"
            visible: !entryMenu.multi && !controller.trashActive
            height: visible ? implicitHeight : 0
            // The bundled star, deliberately not the theme's: this entry has
            // to read as the same mark the badge draws on the tile, and not
            // every theme carries both a filled and an outline star.
            icon.source: CelestinaTheme.fallbackIcon(
                             mainPanel.isFavorite(entryMenu.targetPath)
                             ? "star" : "star-outline")
            onTriggered: controller.toggleFavorite(entryMenu.targetPath)
        }

        GlassMenuItem {
            text: "Renombrar"
            visible: !entryMenu.multi && !controller.trashActive
            height: visible ? implicitHeight : 0
            icon.name: "edit-rename"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: namePrompt.openRename(entryMenu.targetPath, entryMenu.targetName)
        }

        GlassMenuItem {
            text: "Renombrar " + entryMenu.actingCount + " elementos…"
            visible: entryMenu.multi && !controller.trashActive
            height: visible ? implicitHeight : 0
            icon.name: "edit-rename"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: batchRename.open(
                             mainPanel.operativePaths(entryMenu.targetToken,
                                                      entryMenu.targetPath))
        }

        GlassMenuItem {
            text: entryMenu.multi
                  ? "Copiar " + entryMenu.actingCount + " elementos"
                  : "Copiar"
            visible: !controller.trashActive
            height: visible ? implicitHeight : 0
            icon.name: "edit-copy"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: mainPanel.copySelection(
                             entryMenu.targetToken, entryMenu.targetPath, false)
        }

        GlassMenuItem {
            text: entryMenu.multi
                  ? "Cortar " + entryMenu.actingCount + " elementos"
                  : "Cortar"
            visible: !controller.trashActive
            height: visible ? implicitHeight : 0
            icon.name: "edit-cut"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: mainPanel.copySelection(
                             entryMenu.targetToken, entryMenu.targetPath, true)
        }

        GlassMenuItem {
            text: entryMenu.multi
                  ? "Enviar " + entryMenu.actingCount + " a la papelera"
                  : "Enviar a la papelera"
            visible: !controller.trashActive
            height: visible ? implicitHeight : 0
            icon.name: "user-trash"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: mainPanel.trashSelection(
                             entryMenu.targetToken, entryMenu.targetPath)
        }

        GlassMenuItem {
            text: "Cambiar icono…"
            visible: !entryMenu.multi && !controller.trashActive
            height: visible ? implicitHeight : 0
            icon.name: "preferences-desktop-icons"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: iconPicker.openFor(entryMenu.targetPath, entryMenu.targetDirectory)
        }

        GlassMenuItem {
            text: "Propiedades"
            visible: !entryMenu.multi
            height: visible ? implicitHeight : 0
            icon.name: "document-properties"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: controller.openProperties(entryMenu.targetPath)
        }

        MenuSeparator {
            contentItem: Rectangle {
                implicitHeight: 1
                color: CelestinaTheme.border
            }
        }

        GlassMenuItem {
            text: "Actualizar"
            icon.name: "view-refresh"
            icon.source: CelestinaTheme.fallbackIcon("view-refresh")
            onTriggered: controller.refresh()
        }

        GlassMenuItem {
            text: controller.showHidden
                  ? "Ocultar elementos ocultos"
                  : "Mostrar elementos ocultos"
            onTriggered: controller.toggleHidden()
        }
    }

    // Context menu for the breadcrumb / path bar: act on the current path.
    GlassContextMenu {
        id: pathMenu
        backdropSource: root

        GlassMenuItem {
            text: "Añadir a marcadores"
            icon.name: "bookmark-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: controller.addBookmark(controller.currentPath)
        }

        GlassMenuItem {
            text: "Abrir en pestaña nueva"
            icon.name: "tab-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: root.requestNewTab(controller.currentPath, true)
        }
    }

    GlassContextMenu {
        id: folderMenu
        backdropSource: root

        // Refresh paste availability so "Pegar" also lights up for file
        // URIs another manager placed on the system clipboard.
        onAboutToShow: controller.refreshPasteState()

        GlassMenuItem {
            text: "Nueva carpeta"
            icon.name: "folder-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: namePrompt.openCreate("folder")
        }

        GlassMenuItem {
            text: "Nuevo archivo"
            icon.name: "document-new"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: namePrompt.openCreate("file")
        }

        GlassMenuItem {
            text: "Pegar"
            enabled: controller.canPaste && !controller.opRunning
            icon.name: "edit-paste"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: controller.paste()
        }

        GlassMenuItem {
            text: controller.canUndo ? controller.undoLabel : "Deshacer"
            visible: controller.canUndo
            height: visible ? implicitHeight : 0
            icon.name: "edit-undo"
            icon.source: CelestinaTheme.fallbackIcon("view-refresh")
            onTriggered: controller.undo()
        }

        MenuSeparator {
            contentItem: Rectangle {
                implicitHeight: 1
                color: CelestinaTheme.border
            }
        }

        GlassMenuItem {
            text: "Seleccionar todo"
            onTriggered: mainPanel.selectAll()
        }

        GlassMenuItem {
            text: "Abrir en pestaña nueva"
            icon.name: "tab-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: root.requestNewTab(controller.currentPath, true)
        }

        GlassMenuItem {
            text: "Abrir terminal aquí"
            icon.name: "utilities-terminal"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: controller.openTerminal()
        }

        // Only offered once this folder actually remembers something, and
        // it says plainly what it drops.
        GlassMenuItem {
            text: "Olvidar la vista de esta carpeta"
            visible: controller.folderViewPinned
            height: visible ? implicitHeight : 0
            icon.name: "edit-clear"
            icon.source: CelestinaTheme.fallbackIcon("view-refresh")
            onTriggered: controller.forgetFolderView()
        }

        GlassMenuItem {
            text: "Actualizar"
            icon.name: "view-refresh"
            icon.source: CelestinaTheme.fallbackIcon("view-refresh")
            onTriggered: controller.refresh()
        }

        GlassMenuItem {
            text: controller.showHidden
                  ? "Ocultar elementos ocultos"
                  : "Mostrar elementos ocultos"
            onTriggered: controller.toggleHidden()
        }
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
        Item {
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

            InfoPill {

                textScale: root.hostWindow.interfaceTextScale
                id: searchBarLabel
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                backdrop: topBar.activeView
                iconName: "edit-find"
                iconFallback: "file"
                maxWidth: searchBar.width - searchBarControls.width - 10
                text: controller.searchRunning
                      ? "Buscando «" + controller.searchQuery + "»…"
                      : "«" + controller.searchQuery + "» · " + controller.searchSummary
            }

            Row {
                id: searchBarControls
                anchors.right: parent.right
                anchors.rightMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8

                CelestinaButton {
                    text: "Detener"
                    visible: controller.searchRunning
                    onClicked: controller.cancelSearch()
                }
                CelestinaButton {
                    text: "Cerrar"
                    onClicked: controller.closeSearch()
                }
            }
        }

        // ── Recientes location header ──────────────────────────────────
        // The same shape as the Trash header: a pill that says where you
        // are and how much is here, and the way back. Nothing else — this
        // list belongs to the desktop, and Siderita only reads it.
        Item {
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

            InfoPill {

                textScale: root.hostWindow.interfaceTextScale
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                backdrop: topBar.activeView
                iconName: "document-open-recent"
                iconFallback: "file"
                maxWidth: recentHeader.width - recentHeaderControls.width - 10
                text: "Recientes" + (controller.recentCount > 0
                                     ? "  ·  " + controller.recentCount
                                     : "  ·  sin elementos")
            }

            Row {
                id: recentHeaderControls
                anchors.right: parent.right
                anchors.rightMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8

                CelestinaButton {
                    text: "Volver"
                    primary: true
                    onClicked: controller.closeRecent()
                }
            }
        }

        // ── Trash location header ──────────────────────────────────────
        // Trashed items ride the same entryModel (like search), so the
        // content view renders them as list / grid / details with
        // thumbnails. This slim glass bar floats below the breadcrumb with
        // the bulk actions and the way back.
        Item {
            id: trashHeader
            z: 10
            x: 12
            width: root.width - 24
            height: 40
            y: (tabBar.visible ? tabBar.y + tabBar.height : topBar.y + topBar.height) + 8
            visible: controller.trashActive

            property bool confirmingEmpty: false
            onVisibleChanged: if (!visible) confirmingEmpty = false

            InfoPill {

                textScale: root.hostWindow.interfaceTextScale
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                backdrop: topBar.activeView
                iconName: "user-trash"
                iconFallback: "user-trash"
                maxWidth: trashHeader.width - trashHeaderControls.width - 10
                text: "Papelera" + (controller.trashNames.length > 0
                                    ? "  ·  " + controller.trashNames.length : "  ·  vacía")
            }

            Row {
                id: trashHeaderControls
                anchors.right: parent.right
                anchors.rightMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8

                // Its own pill too: the warning floats over the trash
                // listing, so it needs a surface to be readable on.
                InfoPill {
                    textScale: root.hostWindow.interfaceTextScale
                    visible: trashHeader.confirmingEmpty
                    anchors.verticalCenter: parent.verticalCenter
                    backdrop: topBar.activeView
                    text: "¿Vaciar? No se puede deshacer"
                }
                CelestinaButton {
                    text: trashHeader.confirmingEmpty ? "Vaciar definitivamente" : "Vaciar"
                    destructive: true
                    visible: controller.trashNames.length > 0
                    onClicked: {
                        if (trashHeader.confirmingEmpty) {
                            controller.emptyTrash()
                            trashHeader.confirmingEmpty = false
                        } else {
                            trashHeader.confirmingEmpty = true
                        }
                    }
                }
                CelestinaButton {
                    text: "Restaurar todo"
                    visible: controller.trashNames.length > 0 && !trashHeader.confirmingEmpty
                    onClicked: controller.restoreAllTrash()
                }
                CelestinaButton {
                    text: "Volver"
                    primary: true
                    onClicked: controller.closeTrash()
                }
            }
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
        Item {
            id: detailsHeader
            z: 10
            x: 8
            width: parent.width - 16
            height: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale) + 18
            y: (tabBar.visible ? tabBar.y + tabBar.height : topBar.y + topBar.height) + 8
            visible: fileList.detailsMode

            GlassSurface {
                anchors.fill: parent
                backdropSource: fileList
                captureEnabled: detailsHeader.visible
                liveCapture: true
                cornerRadius: CelestinaTheme.radiusSm
            }

            Rectangle {
                anchors.fill: parent
                radius: CelestinaTheme.radiusSm
                color: "transparent"
                border.width: 1
                border.color: CelestinaTheme.borderStrong
            }

            RowLayout {
                x: fileList.detailsNameX - 4
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - x - 16
                spacing: 12

                Repeater {
                    model: [
                        { label: "Nombre", field: 0, w: -1, align: Text.AlignLeft },
                        { label: "Tamaño", field: 1, w: fileList.colSizeW, align: Text.AlignRight },
                        { label: "Fecha", field: 2, w: fileList.colDateW, align: Text.AlignLeft },
                        { label: "Tipo", field: 3, w: fileList.colTypeW, align: Text.AlignLeft }
                    ]

                    delegate: Item {
                        id: hcell
                        required property var modelData
                        readonly property bool activeSort: controller.sortField === modelData.field
                        Layout.fillWidth: modelData.w < 0
                        Layout.preferredWidth: modelData.w < 0 ? 60 : modelData.w
                        Layout.fillHeight: true

                        Text {
                            anchors.fill: parent
                            verticalAlignment: Text.AlignVCenter
                            horizontalAlignment: hcell.modelData.align
                            text: hcell.modelData.label
                                  + (hcell.activeSort
                                     ? (controller.sortAscending ? "  ↑" : "  ↓") : "")
                            color: hcell.activeSort ? CelestinaTheme.text
                                                    : CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale)
                            font.weight: CelestinaTheme.weightDemiBold
                            elide: Text.ElideRight
                        }

                        MouseArea {
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                if (hcell.activeSort)
                                    controller.toggleSortDirection()
                                else
                                    controller.changeSortField(hcell.modelData.field)
                            }
                        }
                    }
                }
            }
        }
}
