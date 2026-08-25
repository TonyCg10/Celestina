import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.siderita 1.0
import org.celestina.siderita.internal 1.0
// ─── FolderView ───────────────────────────────────────────────────────────────
// Vista de carpeta independiente: ruta, contenido, selección, menús y atajos.
// Cada pestaña conserva su controlador, ubicación e historial; el host sólo
// inyecta arrastre, overlays, estado de ventana y apertura de pestañas.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root
    // El pie usa cristal mientras haya contenido desplazándose detrás.
    readonly property Item bottomView: mainPanel.viewMode === "grid"
                                       ? fileGrid : folderListView
    readonly property bool bottomFloating:
            bottomView && bottomView.contentHeight > bottomView.height
            && !bottomView.atYEnd
    readonly property bool contextualHeaderVisible:
            controller.searchActive || controller.searchRunning
            || controller.trashActive || controller.recentActive
    // El marco sube tras la ruta compacta y baja bajo el encabezado grande.
    readonly property real primaryChromeBottom: folderTabBar.visible
            ? folderTabBar.y + folderTabBar.height : topBar.y + topBar.height
    readonly property real chromeBottom: primaryChromeBottom
            + (contextualHeaderVisible
               ? CelestinaTheme.compFloatingGap + folderChrome.searchBar.height : 0)
    readonly property real expandedFrameY: chromeBottom + CelestinaTheme.compFloatingGap
    readonly property real compactFrameY: topBar.y - CelestinaTheme.compFloatingInset
    readonly property real contentFrameY: expandedFrameY
            + (compactFrameY - expandedFrameY) * folderHeading.compactProgress
    readonly property real contentTopInset: Math.max(0, expandedFrameY - contentFrameY)
    readonly property real contentBottomInset:
            CelestinaTheme.controlHeightSm + 2 * CelestinaTheme.compFloatingInset
    property Item ghost
    property Item overlayParent
    // The window hosting this document: the tab model and size scales are its
    // own, not the tab's.
    property var hostWindow
    property bool active: false
    readonly property bool modalBlocked: folderActions.modalBlocked
    readonly property bool navigationBlocked: folderActions.navigationBlocked
    property alias tabController: controller
    // Nombre distinto evita el auto-binding sombreado `x: x`.
    property alias viewTopBar: topBar
    signal requestNewTab(string path, bool foreground)
    function collapseHeading() { heading.collapse() }
    function restoreHeading() { heading.restore() }
    function retireHeading() { heading.retire() }
    function revealHeading() { heading.reveal() }
    onActiveChanged: if (!active) collapseHeading()
    SideritaController {
        id: controller
    }

    HeadingState {
        id: heading
        canReveal: root.active && !controller.loading && root.bottomView
                   && !controller.errorText.length && root.bottomView.atYBeginning
    }
    // RouteReveal se arma antes de que este modelo publique la ruta nueva.
    // Native role model shared by list and grid.
    SideritaEntryModel {
        id: folderEntryModel
    }
    // Distingue navegación de un refresco del mismo lugar o vista virtual.
    property string renderedKey: ""

    RouteReveal {
        id: routeReveal
        navigationController: root.tabController
        ready: root.renderedKey.length > 0
    }
    Connections {
        target: controller
        function onRowsReady(names, tokens, kinds, subtitles, paths, sections, sizes, dates) {
            var view = mainPanel.viewMode === "grid" ? fileGrid : folderListView
            var key = controller.currentPathKey + "|" + controller.trashActive + "|"
                    + controller.recentActive + "|" + controller.searchActive + "|"
                    + controller.searchQuery
            var samePlace = key === root.renderedKey
            var savedY = view.contentY

            folderEntryModel.setRows(names, tokens, kinds, subtitles, paths, sections, sizes, dates)
            root.renderedKey = key

            // setRows reinicia el modelo, y un reset manda la vista al top (que
            // con el margen superior es contentY = -topMargin, no 0). En la misma
            // ubicación —un refresco del watch— eso te arranca de donde leías, así
            // que le devolvemos su contentY (la vista lo acota a rango sola). Al
            // navegar no se toca: ir al top es lo correcto para una carpeta nueva.
            if (samePlace)
                view.contentY = savedY

            routeReveal.revealPreparedRoute()
        }
    }

    // The overlay (below) previews the selected entry; ↑/↓ step the selection,
    // and focus returns to the active view on close.
    property bool quickLookOpen: false
    onQuickLookOpenChanged: if (!quickLookOpen) {
        if (mainPanel.viewMode === "grid")
            fileGrid.forceActiveFocus()
        else
            folderListView.forceActiveFocus()
    }
    // Devuelve el foco a la vista activa; lo usan los diálogos al cerrarse.
    // Antes fijaban la lista aunque la rejilla estuviera activa.
    function focusView() {
        if (mainPanel.viewMode === "grid")
            fileGrid.forceActiveFocus()
        else
            folderListView.forceActiveFocus()
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
            folderListView.selectRow(j)
    }

    // Drop any active search and empty the field. Fired on navigation, so a
    // place click lands on a clean folder, and by the mouse Back button.
    function clearSearch() {
        topBar.searchText = ""
        controller.applyQuery("")
        controller.closeSearch()
    }

    // Back undoes wherever you are, in the order you got there: a search
    // bows out first, then the Trash location, and only then does history
    // move. Trash is a place you can be but not a folder in history.
    // `loading` holds back only the history move, the one step that races a
    // scan; leaving a location cancels its reading instead of waiting for it,
    // and gating that on `loading` locked the person inside a slow one.
    readonly property bool canGoBackOrLeave:
            topBar.searchText.length > 0 || controller.searchActive
            || controller.trashActive || controller.recentActive
            || (!controller.loading && controller.canGoBack)

    function goBackOrLeave() {
        if (topBar.searchText.length > 0 || controller.searchActive)
            clearSearch()
        else if (controller.trashActive)
            controller.closeTrash()
        else if (controller.recentActive)
            controller.closeRecent()
        else if (!controller.loading && controller.canGoBack)
            controller.goBack()
    }

    FolderShortcuts {
        anchors.fill: parent
        viewActive: root.active && !root.navigationBlocked
        canGoBackOrLeave: root.canGoBackOrLeave
        controller: tabController
        panel: mainPanel
        topBar: viewTopBar
        namePrompt: folderActions.namePrompt
        onGoBackOrLeaveRequested: root.goBackOrLeave()
    }

    EntryIconRules {
        id: entryIconRules
        controller: root.tabController
        hostWindow: root.hostWindow
    }

    Item {
        id: mainPanel

        property string viewMode: "list"   // "list" | "grid"
        onViewModeChanged: root.collapseHeading()

        // Restore the last-used view mode on open and persist a change
        // (list⇄grid). The size scales are window-level and independent.
        Component.onCompleted: {
            viewMode = controller.savedViewMode()
            entryIconRules.rebuildFolderTypeIcons()
        }
        // Picking a view is both a new default and a statement about this
        // folder: the global keeps folders you have never arranged looking
        // the way you last chose, and this folder now remembers its own.
        function persist() {
            controller.saveViewMode(viewMode)
            controller.rememberViewMode(viewMode)
        }

        // A folder that remembers a view wins over the global default when it
        // opens; empty means never arranged, and leaves the current view alone.
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
            CelestinaTheme.rowHeightLg,
            Math.max(
                Math.round(CelestinaTheme.glyphTile
                           * root.hostWindow.contentIconScale),
                Math.round((CelestinaTheme.fontBody + CelestinaTheme.fontCaption)
                           * 1.35 * root.hostWindow.contentTextScale)) + 16)
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
        // Appearance rules live in their own object, declared at the view's own
        // level so it exists before anything binds to it; the panel only
        // republishes it so delegates and dialogs reach it without borrowing an
        // id from outside their own scope.
        readonly property var icons: entryIconRules

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
        // The dropped URLs as plain strings, for controller.dropUriList. Not
        // decoded here: `decodeURIComponent` throws a URIError on an escape
        // that is not valid UTF-8 — how another manager spells a non-UTF-8
        // name — and that took the whole batch down in silence. Rust decodes
        // by bytes and skips only what it cannot resolve.
        function droppedUris(urls) {
            var out = []
            for (var i = 0; i < urls.length; i++)
                out.push(urls[i].toString())
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
                controller.dropUriList(droppedUris(drop.urls), destPath,
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
                root.collapseHeading()
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
            function onSearchActiveChanged() {
                root.collapseHeading()
                mainPanel.clearSelection()
            }
            function onSearchQueryChanged() { root.collapseHeading() }
            function onTrashActiveChanged() { root.collapseHeading() }
            function onRecentActiveChanged() { root.collapseHeading() }
            function onLoadingChanged() {
                if (controller.loading)
                    root.collapseHeading()
            }
        }

        anchors.fill: parent

        readonly property real contentFrameY: root.contentFrameY
        readonly property real contentFrameX: contentFrame.frameX
        readonly property real contentFrameWidth: contentFrame.frameWidth
        readonly property real contentFrameBottom: contentFrame.frameBottom
        readonly property real floatingChromeInset:
                CelestinaTheme.compFloatingInset
        readonly property real floatingChromeX:
                contentFrameX + floatingChromeInset
        readonly property real floatingChromeWidth: Math.max(
                0, contentFrameWidth - 2 * floatingChromeInset)
        readonly property real contentTopInset: root.contentTopInset
        readonly property real contentRowsY: contentFrameY + contentTopInset
        readonly property real contentBottomInset: root.contentBottomInset

        FolderContentFrame {
            id: contentFrame
            anchors.fill: parent
            frameY: root.contentFrameY
        }

        // Right-click on empty space (behind the views) opens the folder
        // menu; right-clicks that land on an item are handled by the item.
        MouseArea {
            id: emptySpaceMouse
            x: contentFrame.surface.x
            y: contentFrame.surface.y
            width: contentFrame.surface.width
            height: contentFrame.surface.height
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
            x: contentFrame.surface.x
            y: contentFrame.surface.y
            width: contentFrame.surface.width
            height: Math.max(0, contentFrame.surface.height
                             - mainPanel.contentBottomInset)
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
                controller.dropUriList(mainPanel.droppedUris(drop.urls),
                                       "", mainPanel.dropIsMove(drop))
                drop.accept()
            }

            Rectangle {
                anchors.fill: parent
                visible: viewDrop.containsDrag
                color: CelestinaTheme.clear
                border.width: CelestinaTheme.borderFocus
                border.color: CelestinaTheme.accent
                radius: contentFrame.surfaceRadius
                z: 40
            }
        }

        FolderListView {
            id: folderListView
            x: contentFrame.surface.x
            y: contentFrame.surface.y
            width: contentFrame.surface.width
            height: contentFrame.surface.height
            opacity: routeReveal.progress
            scale: routeReveal.revealScale
            controller: tabController
            entryModel: folderEntryModel
            panel: mainPanel
            hostWindow: root.hostWindow
            ghost: root.ghost
            overlayParent: root.overlayParent
            contentTopMargin: mainPanel.contentTopInset
                              + (folderListView.detailsMode
                                 ? folderChrome.detailsHeader.height + 12 : 8)
            contentBottomInset: mainPanel.contentBottomInset
            headingState: heading
            onRevealHeadingRequested: root.revealHeading()
            onRestoreHeadingRequested: root.restoreHeading()
            onCollapseHeadingRequested: root.collapseHeading()
            onRetireHeadingRequested: root.retireHeading()
            onQuickLookRequested: folderActions.requestPreview()
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
            x: contentFrame.surface.x
            y: contentFrame.surface.y
            width: contentFrame.surface.width
            height: contentFrame.surface.height
            opacity: routeReveal.progress
            scale: routeReveal.revealScale
            controller: tabController
            entryModel: folderEntryModel
            panel: mainPanel
            hostWindow: root.hostWindow
            ghost: root.ghost
            overlayParent: root.overlayParent
            contentTopMargin: mainPanel.contentTopInset + 8
            contentBottomInset: mainPanel.contentBottomInset
            headingState: heading
            onRevealHeadingRequested: root.revealHeading()
            onRestoreHeadingRequested: root.restoreHeading()
            onCollapseHeadingRequested: root.collapseHeading()
            onRetireHeadingRequested: root.retireHeading()
            onQuickLookRequested: folderActions.requestPreview()
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
            x: contentFrame.surface.x
            y: contentFrame.surface.y
            width: contentFrame.surface.width
            height: contentFrame.surface.height
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
                root.focusView()   // the view on screen, not always the list
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

        FolderEmptyState {
            x: contentFrame.surface.x
               + (contentFrame.surface.width - width) / 2
            y: contentFrame.surface.y
               + (contentFrame.surface.height - height) / 2
            opacity: routeReveal.progress
            scale: routeReveal.revealScale
            controller: tabController
        }

        FolderBottomChrome {
            z: 20
            anchors.fill: parent
            controller: tabController
            hostWindow: root.hostWindow
            panel: mainPanel
            contentSurface: contentFrame.surface
            bottomView: root.bottomView
            bottomFloating: root.bottomFloating
            overlayParent: root.overlayParent
            sortMenuItem: folderActions.sortMenu
        }
    }

    FolderHeading {
        id: folderHeading
        z: 9
        x: CelestinaTheme.compFloatingInset
        y: x * (1 - retiredProgress)
        width: root.width - 2 * x
        opacity: routeReveal.progress; scale: routeReveal.revealScale
        compact: !heading.expanded
        retired: heading.retired
        controller: tabController
        hostWindow: root.hostWindow
        shortcutActive: root.active && !root.navigationBlocked
        onPhoneMediaRequested: index => folderActions.openPhoneMedia(index)
    }

    PhoneMediaUnderBar {
        bar: topBar; heading: folderHeading
        onClicked: folderActions.openPhoneMedia(folderHeading.phoneIndex)
    }

    TopBar {
        id: topBar
        z: 10
        headingRetired: folderHeading.retiredProgress
        phoneLocation: folderHeading.phoneLocation
        phoneConnected: folderHeading.phoneConnected
        phoneIndex: folderHeading.phoneIndex
        onPhoneMediaRequested: index => folderActions.openPhoneMedia(index)
        x: mainPanel.floatingChromeX
        y: folderHeading.y + folderHeading.height + CelestinaTheme.spaceLg
           * (1 - folderHeading.retiredProgress) + CelestinaTheme.compFloatingInset
           * folderHeading.retiredProgress
        width: mainPanel.floatingChromeWidth
        height: CelestinaTheme.controlHeightLg
        // No route reveal here: fading the breadcrumb and the search field —
        // the very controls that carry you between routes — reads as a flinch.
        controller: tabController
        activeView: mainPanel.viewMode === "grid" ? fileGrid : folderListView
        hostWindow: root.hostWindow
        overlayParent: root.overlayParent
        pathMenu: folderActions.pathMenu
        onViewFocusRequested: root.focusView()
    }

    // A second contextual row appears only when there is somewhere to switch.
    // Each tab paints its own pill; the strip itself has no enclosing box.
    TabStrip {
        id: folderTabBar
        z: 10
        x: mainPanel.floatingChromeX
        y: topBar.y + topBar.height + CelestinaTheme.compFloatingGap
        width: mainPanel.floatingChromeWidth
        height: 36
        visible: root.hostWindow !== undefined
                 && root.hostWindow.tabsModel.count >= 2
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
        tabBar: folderTabBar
        fileList: folderListView
    }
}
