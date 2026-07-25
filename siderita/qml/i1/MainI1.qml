import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import org.celestina.siderita 1.0
import org.celestina.siderita.internal 1.0

ApplicationWindow {
    id: window

    width: 1120
    height: 720
    minimumWidth: 680
    minimumHeight: 480
    // Shown by Component.onCompleted, once the remembered size is on and the
    // session's tabs exist — so the window never flashes at the default size
    // and then jumps.
    visible: false
    color: CelestinaTheme.canvas
    title: "Siderita · Iteración 1"

    // ── Session ──────────────────────────────────────────────────────────
    // A controller that owns no tab, used only to read and write what belongs
    // to the window rather than to any one tab: its size and the set of open
    // tabs. It is never started, so it runs no scan and holds no watch — it is
    // the settings store the window needs before the first tab exists.
    SideritaController {
        id: sessionStore
    }

    function persistSession() {
        const paths = []
        for (var i = 0; i < tabRepeater.count; i++) {
            const holder = tabRepeater.itemAt(i)
            if (!holder)
                continue
            // A tab that has not been shown yet has no current path — it still
            // belongs to the session, under the folder it was restored with.
            const path = holder.docController && holder.docController.currentPath.length > 0
                       ? holder.docController.currentPath
                       : holder.initialPath
            if (path.length > 0)
                paths.push(path)
        }
        if (paths.length > 0)
            sessionStore.saveTabs(paths, window.currentTabIndex)
    }

    // Both are chatty while a window is dragged or a folder is loading, so they
    // settle first and write once.
    Timer {
        id: sessionSaver
        interval: 700
        onTriggered: window.persistSession()
    }

    Timer {
        id: geometrySaver
        interval: 700
        onTriggered: sessionStore.saveWindowSize(window.width, window.height)
    }

    onWidthChanged: if (visible) geometrySaver.restart()
    onHeightChanged: if (visible) geometrySaver.restart()

    // ── Tabs ─────────────────────────────────────────────────────────────
    // Each tab is an independent FolderView with its own SideritaController
    // (history, scan worker, selection). The window chrome — sidebar and tab
    // strip — binds to the active tab's controller via `activeController`.
    property int currentTabIndex: 0
    // Bumped whenever the tab Repeater adds/removes an item, so the
    // `activeController` binding re-resolves itemAt() after the delegate exists.
    property int tabsRevision: 0
    readonly property var activeController: {
        tabsRevision // re-evaluate when tabs are created or destroyed
        const holder = tabRepeater.itemAt(currentTabIndex)
        return holder ? holder.docController : null
    }

    // ── Granular size scales (window-level so every tab and the sidebar share
    // one persisted set) ─────────────────────────────────────────────────────
    // Four independent zoom factors, loaded once a controller exists and saved
    // on any change. Content scales drive the per-tab list/grid/search; sidebar
    // scales drive the shared sidebar and its info box.
    property real contentIconScale: 1.0
    property real contentTextScale: 1.0
    property real interfaceIconScale: 1.0
    property real interfaceTextScale: 1.0
    property real sidebarIconScale: 1.0
    property real sidebarTextScale: 1.0
    property bool sizingLoaded: false

    // Sidebar rows grow to fit whichever of their icon or label is taller, so
    // the two sidebar sliders never clip each other.
    readonly property int sidebarRowHeight: Math.max(
        Math.round(CelestinaTheme.iconSm * sidebarIconScale) + 16,
        Math.round(CelestinaTheme.fontBody * sidebarTextScale) + 21)

    function loadSizing() {
        if (sizingLoaded || !activeController)
            return
        contentIconScale = activeController.savedContentIconScale()
        contentTextScale = activeController.savedContentTextScale()
        interfaceIconScale = activeController.savedInterfaceIconScale()
        interfaceTextScale = activeController.savedInterfaceTextScale()
        sidebarIconScale = activeController.savedSidebarIconScale()
        sidebarTextScale = activeController.savedSidebarTextScale()
        sizingLoaded = true
    }

    function persistSizing() {
        if (activeController)
            activeController.saveSizing(contentIconScale, contentTextScale,
                                        interfaceIconScale, interfaceTextScale,
                                        sidebarIconScale, sidebarTextScale)
    }

    function tabTitle(p) {
        if (!p || p.length === 0)
            return "…"
        if (p === "/")
            return "/"
        const s = p.replace(/\/+$/, "")
        const i = s.lastIndexOf("/")
        return i >= 0 ? s.substring(i + 1) : s
    }

    function openTab(path, foreground) {
        const initial = (path === undefined || path === null) ? "" : path
        tabsModel.append({ initialPath: initial, title: window.tabTitle(initial) })
        if (foreground)
            window.currentTabIndex = tabsModel.count - 1
    }

    function closeTab(i) {
        if (tabsModel.count <= 1 || i < 0 || i >= tabsModel.count)
            return
        tabsModel.remove(i)
        if (window.currentTabIndex >= tabsModel.count)
            window.currentTabIndex = tabsModel.count - 1
        else if (i < window.currentTabIndex)
            window.currentTabIndex = window.currentTabIndex - 1
    }

    function selectTab(i) {
        if (i >= 0 && i < tabsModel.count)
            window.currentTabIndex = i
    }

    function cycleTab(delta) {
        if (tabsModel.count <= 1)
            return
        window.currentTabIndex =
            (window.currentTabIndex + delta + tabsModel.count) % tabsModel.count
    }

    // A freshly-activated tab may have been created before a bookmark, an icon
    // override or a star was set in another tab; re-read the shared files so its
    // sidebar and its entries are truthful.
    onCurrentTabIndexChanged: {
        sessionSaver.restart()
        if (window.activeController) {
            window.activeController.reloadBookmarks()
            window.activeController.reloadCustomIcons()
            window.activeController.reloadFavorites()
            window.activeController.reloadPlaces()
        }
    }

    // How to draw each sidebar place key. The controller owns which of these
    // exist and in what order; this is only the label and the glyph.
    readonly property var placeDefs: ({
        "HOME":      { name: "Inicio",     icon: "user-home",        fallback: "go-home" },
        "DESKTOP":   { name: "Escritorio", icon: "user-desktop",     fallback: "folder" },
        "DOCUMENTS": { name: "Documentos", icon: "folder-documents", fallback: "folder" },
        "DOWNLOAD":  { name: "Descargas",  icon: "folder-download",  fallback: "folder" },
        "MUSIC":     { name: "Música",     icon: "folder-music",     fallback: "folder" },
        "PICTURES":  { name: "Imágenes",   icon: "folder-pictures",  fallback: "folder" },
        "VIDEOS":    { name: "Vídeos",     icon: "folder-videos",    fallback: "folder" },
        "RECENT":    { name: "Recientes",  icon: "document-open-recent", fallback: "file" },
        "TRASH":     { name: "Papelera",   icon: "user-trash",       fallback: "user-trash" }
    })

    // Exposed as a property (not merely an id) so each per-tab FolderView can
    // reach it through `tabHost`; a plain child id is invisible as
    // `window.tabsModel`.
    property alias tabsModel: tabsModelData

    ListModel {
        id: tabsModelData
    }

    // ── FavoriteBadge ────────────────────────────────────────────────────
    // The star a favourited entry wears, sat in the corner of its tile. It
    // carries its own dark disc because it has to stay readable over a folder
    // glyph, a photo thumbnail and an empty tile alike.
    // ── GlassPill ────────────────────────────────────────────────────────
    // La pastilla del pie: cristal debajo, tinte de estado encima. El orden
    // importa — los tokens de relleno son translúcidos, así que el tinte deja
    // ver el desenfoque en vez de taparlo, y una pastilla activa sigue siendo
    // reconocible sobre el contenido que pasa por debajo.

    // El galón de las cabeceras del sidebar. Una sola punta que gira en vez de
    // dos glifos que se intercambian: el giro *cuenta* que la zona se abre, y el
    // salto entre "▸" y "▾" no cuenta nada.

    // Themed push button for dialogs/overlays (the default QtQuick Basic Button
    // is unstyled). `primary` fills with the accent; otherwise a control-fill
    // pill with a border.

    // ── InfoPill ─────────────────────────────────────────────────────────
    // A glass pill that hugs its own label, sized and shaped like a
    // PillButton. The floating strips (the search summary, the Trash header)
    // are built from these instead of one bar spanning the window, so a
    // header reads as a few independent pills — the label is one, each action
    // is its own — and the content behind shows through between them.

    // One rule field of the batch-rename dialog — the suite's input styling in
    // the one place four of them sit side by side.

    // ── DragScrollEdge ───────────────────────────────────────────────────
    // A thin strip over the top or bottom of a view: while a drag rests on it,
    // the view scrolls, so a destination below the fold does not mean dropping
    // the entry somewhere else first and picking it up again. It sits above the
    // rows (a row would otherwise swallow the drag), so a release on it means
    // the *current* folder — the same thing an empty-space drop means — and an
    // entry dragged within its own folder simply has nowhere to go.

    // A labelled zoom row for the sizing popup: caption · slider · percent.
    // The consumer binds `value` to a scale and updates it in `onMoved`.

    // One label : value line in the properties panel; hides itself when empty.

    // App-global org.freedesktop.FileManager1 service: "Show in file manager"
    // from another application opens the folder in a new foreground tab and
    // raises the window. One instance for the whole window, not per tab.
    FileManager1Service {
        id: fileManager1
        Component.onCompleted: fileManager1.start()
        onOpenFolderRequested: function(path) {
            window.openTab(path, true)
            window.requestActivate()
        }
    }

    // ── The desktop's file chooser ───────────────────────────────────────
    // `org.freedesktop.impl.portal.FileChooser`: when the session routes that
    // interface here, every "open a file" and "save as" dialog in every portal-
    // using application becomes a picker window of ours. Each request gets its
    // own window — several applications can be asking at once — and the window
    // answers the waiting D-Bus call through `portalService.answer`.
    FileChooserPortal {
        id: portalService
        Component.onCompleted: portalService.start()

        onPickRequested: function(token, mode, appId, title, acceptLabel,
                                  multiple, directory, currentFolder,
                                  currentName, filters) {
            // `filters` is a plain JS array, so it rides in a property rather
            // than a ListModel role (a role would flatten it to a string).
            pickerFilters[token] = filters
            pickerRequests.append({
                token: token, mode: mode, appId: appId, title: title,
                acceptLabel: acceptLabel, multiple: multiple,
                directory: directory, currentFolder: currentFolder,
                currentName: currentName
            })
        }

        // The asking application went away: close that picker without answering
        // (the call it was waiting on is already gone).
        onPickWithdrawn: function(token) { window.dropPicker(token) }

        // The answer reached the caller — now the dialog may go.
        onPickAnswered: function(token) { window.dropPicker(token) }
    }

    ListModel {
        id: pickerRequests
    }

    // Per-request filter lists, keyed by token.
    property var pickerFilters: ({})

    function dropPicker(token) {
        delete pickerFilters[token]
        for (var i = 0; i < pickerRequests.count; i++) {
            if (pickerRequests.get(i).token === token) {
                pickerRequests.remove(i)
                return
            }
        }
    }

    Instantiator {
        model: pickerRequests
        // The roles come in through `model`, not as re-declared properties:
        // re-declaring them here would shadow the window's own required ones
        // (and `title` would collide with Window.title), leaving them unset and
        // the delegate uncreatable.
        delegate: PickerWindow {
            required property var model

            token: model.token
            mode: model.mode
            appId: model.appId
            acceptLabel: model.acceptLabel
            multiple: model.multiple
            directory: model.directory
            requestTitle: model.title
            startFolder: model.currentFolder
            suggestedName: model.currentName
            filters: window.pickerFilters[model.token] || []
        }
    }

    // Load the removable-volume list into whichever tab is active — once the
    // first controller resolves, and again on each tab switch — so the
    // window-scope "Dispositivos" sidebar always reflects the active tab.
    Connections {
        target: window
        function onActiveControllerChanged() {
            if (window.activeController) {
                window.activeController.loadVolumes()
                window.loadSizing()
            }
        }
    }

    // ── Window-level tab management shortcuts ────────────────────────────
    Shortcut {
        sequence: "Ctrl+T"
        onActivated: window.openTab(window.activeController
                                    ? window.activeController.currentPath : "", true)
    }

    Shortcut {
        sequence: "Ctrl+W"
        enabled: tabsModel.count > 1
        onActivated: window.closeTab(window.currentTabIndex)
    }

    Shortcut {
        sequence: "Ctrl+Tab"
        enabled: tabsModel.count > 1
        onActivated: window.cycleTab(1)
    }

    Shortcut {
        sequence: "Ctrl+Shift+Tab"
        enabled: tabsModel.count > 1
        onActivated: window.cycleTab(-1)
    }

    Shortcut {
        sequence: "Ctrl+PgDown"
        enabled: tabsModel.count > 1
        onActivated: window.cycleTab(1)
    }

    Shortcut {
        sequence: "Ctrl+PgUp"
        enabled: tabsModel.count > 1
        onActivated: window.cycleTab(-1)
    }

    Item {
        id: contentLayer
        anchors.fill: parent

        Rectangle {
            anchors.fill: parent
            color: CelestinaTheme.canvas

            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: CelestinaTheme.gradientStart }
                GradientStop { position: 0.55; color: CelestinaTheme.gradientMid }
                GradientStop { position: 1; color: CelestinaTheme.gradientEnd }
            }
        }

        // The drag carrier: it holds the Drag state (keys, mime, image) for an
        // entry drag but is never drawn itself — under Drag.Automatic the
        // compositor renders the pixmap set in Drag.imageSource (the entry's
        // grabbed icon; see mainPanel.startEntryDrag), which correctly tracks the
        // cursor. `path` is read back by the drop handlers.
        Item {
            id: dragGhost
            visible: false
            property string path: ""
            property string label: ""
            property bool isDir: false
            // Automatic so the drag also reaches other applications as a
            // text/uri-list; internal DropAreas still match on our keys.
            Drag.dragType: Drag.Automatic
            Drag.supportedActions: Qt.CopyAction | Qt.MoveAction

            // Prime the drag payload (keys + uri-list); the caller sets the drag
            // image and flips Drag.active. Only folders carry the bookmark key,
            // so a file can't be dropped on the sidebar; every entry carries the
            // move key for folder-to-folder drops and a file:// URI for other apps.
            function beginEntryDrag(entryPath, entryLabel, entryIsDir) {
                dragGhost.path = entryPath
                dragGhost.label = entryLabel
                dragGhost.isDir = entryIsDir
                dragGhost.Drag.keys = entryIsDir
                    ? ["siderita-entry", "siderita-bookmark"]
                    : ["siderita-entry"]
                dragGhost.Drag.mimeData = {
                    "text/uri-list": "file://" + encodeURI(entryPath) + "\r\n"
                }
            }
        }

        Rectangle {
            id: sidebar
            x: 20
            y: 18
            width: 184
            // Leave room below for the separate item-info box (its height scales
            // with the sidebar text) plus a gap.
            height: parent.height - y - 18 - sidebarInfo.height - 14
            radius: CelestinaTheme.radiusLg
            visible: parent.width >= 820
            color: CelestinaTheme.surface
            border.width: 1
            border.color: CelestinaTheme.border

            DropArea {
                anchors.fill: parent
                keys: ["siderita-bookmark"]
                onDropped: {
                    if (window.activeController)
                        window.activeController.addBookmark(dragGhost.path)
                }

                Rectangle {
                    anchors.fill: parent
                    radius: sidebar.radius
                    color: "transparent"
                    border.width: 2
                    border.color: CelestinaTheme.accent
                    visible: parent.containsDrag
                }
            }

            Text {
                x: 16
                y: 16
                text: "SIDERITA"
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                font.letterSpacing: 1.7
                font.weight: CelestinaTheme.weightDemiBold
            }

            // Cada zona se pliega por su cabecera. Vive en la sesión y no en
            // disco a propósito: plegar es un gesto de "ahora estorba", no una
            // preferencia — al abrir Siderita el panel enseña lo que hay.
            property bool placesCollapsed: false
            property bool devicesCollapsed: false
            property bool favoritesCollapsed: false
            property bool bookmarksCollapsed: false

            // ── Favourites ───────────────────────────────────────────────
            // Where the stars land. A starred folder opens; a starred file
            // reveals itself in its folder rather than launching an app from a
            // single click. The section disappears entirely when nothing is
            // starred, so the sidebar costs nothing until it is used.
            readonly property var favoriteRows: {
                var rows = []
                var ac = window.activeController
                var entries = ac ? ac.favoriteEntries : []
                for (var i = 0; i < entries.length; i++) {
                    var cut = entries[i].indexOf("\t")
                    if (cut <= 0)
                        continue
                    var path = entries[i].substring(0, cut)
                    var slash = path.lastIndexOf("/")
                    rows.push({
                        path: path,
                        kind: entries[i].substring(cut + 1),
                        name: slash >= 0 && slash < path.length - 1
                              ? path.substring(slash + 1) : path
                    })
                }
                return rows
            }

            // Todo el sidebar se desplaza: las secciones crecen con lo que el
            // usuario guarda (marcadores, favoritos, dispositivos) y antes la
            // última se comía el espacio de las demás. Una sola barra para el
            // panel entero — las listas de dentro no se desplazan solas.
            Flickable {
                id: sidebarScroll
                x: 0
                y: 42
                width: parent.width
                height: parent.height - y - 10
                clip: true
                contentWidth: width
                contentHeight: bookmarksList.y + bookmarksList.height + 14
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

                Column {
                    id: placesColumn
                    x: 8
                    y: 0
                    width: parent.width - 16
                    spacing: 2

                    // La cabecera que faltaba: sin ella los lugares eran la única
                    // zona sin nombre y la única que no se podía plegar.
                    Item {
                        width: placesColumn.width
                        height: 22

                        SidebarChevron {

                            textScale: window.sidebarTextScale
                            x: -4
                            y: 9
                            collapsed: sidebar.placesCollapsed
                        }

                        Text {
                            x: 8
                            y: 8
                            text: "LUGARES"
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontMini * window.sidebarTextScale)
                            font.letterSpacing: 1.4
                            font.weight: CelestinaTheme.weightDemiBold
                        }

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: sidebar.placesCollapsed = !sidebar.placesCollapsed
                        }
                    }

                    // ── Places ───────────────────────────────────────────────
                    // The list, its order and what it leaves out all come from the
                    // controller: `placeKeys` is what exists here, arranged the way
                    // the user arranged it, minus what they hid. This side only
                    // knows how to draw a key.
                    ListView {
                        id: placesList
                        width: placesColumn.width
                        height: sidebar.placesCollapsed ? 0 : count * rowPitch
                        visible: !sidebar.placesCollapsed
                        spacing: 2
                        // Short and bounded — the sidebar scrolls, this never does,
                        // and a non-interactive list leaves the wheel to the sidebar.
                        interactive: false
                        model: window.activeController
                               ? window.activeController.placeKeys : []


                        readonly property int rowPitch: window.sidebarRowHeight + spacing
                        property int dragIndex: -1
                        property int dropIndex: -1

                        function moveDragged(from, to) {
                            dragIndex = -1
                            dropIndex = -1
                            // Last: the move republishes placeKeys, which resets this
                            // view and destroys the delegate that called us.
                            if (to >= 0 && to !== from && window.activeController)
                                window.activeController.movePlace(from, to)
                        }

                        delegate: Item {
                            id: placeRow

                            required property int index
                            required property string modelData      // the place key

                            readonly property var def: window.placeDefs[modelData]
                                                       || ({ name: modelData, icon: "folder",
                                                             fallback: "folder" })
                            readonly property bool isTrash: modelData === "TRASH"
                            readonly property bool isRecent: modelData === "RECENT"
                            // Trash and Recientes are locations, not folders: they
                            // have no path to open, they flip a state instead.
                            readonly property string placePath:
                                    isTrash || isRecent || !window.activeController
                                    ? "" : window.activeController.placePath(modelData)
                            readonly property bool current: isTrash
                                    ? (window.activeController
                                       && window.activeController.trashActive)
                                    : isRecent
                                    ? (window.activeController
                                       && window.activeController.recentActive)
                                    : (placePath.length > 0
                                       && placePath === (window.activeController
                                                         ? window.activeController.currentPath : ""))
                            readonly property bool dragging: placesList.dragIndex === index
                            property bool justDragged: false

                            width: placesList.width
                            height: window.sidebarRowHeight
                            z: dragging ? 2 : 0

                            Accessible.role: Accessible.Button
                            Accessible.name: def.name
                            Accessible.onPressAction: placeRow.activate()

                            function activate() {
                                const ac = window.activeController
                                if (!ac)
                                    return
                                if (isTrash)
                                    ac.openTrash()
                                else if (isRecent)
                                    ac.openRecent()
                                else if (placePath.length > 0)
                                    ac.openLocation(placePath)
                            }

                            // Where the carried row would land.
                            Rectangle {
                                z: 3
                                visible: placesList.dragIndex >= 0
                                         && placesList.dragIndex !== placeRow.index
                                         && placesList.dropIndex === placeRow.index
                                x: 2
                                width: parent.width - 4
                                height: 2
                                radius: 1
                                y: placesList.dropIndex > placesList.dragIndex
                                   ? parent.height - height : 0
                                color: CelestinaTheme.accent
                            }

                            Item {
                                id: placeContent
                                width: placeRow.width
                                height: placeRow.height
                                opacity: placeRow.dragging ? 0.9 : 1

                                // Eases back into place when a drag ends where it
                                // started; a drag that did move is replaced by the
                                // republished list anyway.
                                Behavior on y {
                                    enabled: !placeMouse.drag.active
                                    NumberAnimation {
                                        duration: CelestinaTheme.motionFast
                                        easing.type: CelestinaTheme.easeStandard
                                    }
                                }

                                Rectangle {
                                    anchors.fill: parent
                                    anchors.leftMargin: 2
                                    anchors.rightMargin: 2
                                    radius: CelestinaTheme.radiusSm
                                    color: placeRow.dragging
                                           ? CelestinaTheme.surfaceStrong
                                           : placeRow.current
                                             ? CelestinaTheme.badgeAccentFill
                                             : placeMouse.containsMouse
                                               ? CelestinaTheme.surfaceHover
                                               : "transparent"

                                    Behavior on color {
                                        ColorAnimation { duration: CelestinaTheme.motionFast }
                                    }
                                }

                                IconImage {
                                    id: placeIcon
                                    x: 12
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                                    height: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                                    name: placeRow.def.icon
                                    source: CelestinaTheme.fallbackIcon(placeRow.def.fallback)
                                    // Native theme colours (no tint).
                                }

                                Text {
                                    x: placeIcon.x + placeIcon.width + 10
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: parent.width - x - 12
                                    text: placeRow.def.name
                                    color: placeRow.current ? CelestinaTheme.accent
                                                            : CelestinaTheme.text
                                    font.family: CelestinaTheme.sansFamily
                                    font.pixelSize: Math.round(CelestinaTheme.fontBody * window.sidebarTextScale)
                                    font.weight: placeRow.current ? CelestinaTheme.weightMedium
                                                                  : CelestinaTheme.weightRegular
                                    elide: Text.ElideRight
                                }

                                MouseArea {
                                    id: placeMouse
                                    anchors.fill: parent
                                    acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    drag.target: placeContent
                                    drag.axis: Drag.YAxis
                                    drag.smoothed: false
                                    drag.threshold: 6
                                    drag.minimumY: -placeRow.index * placesList.rowPitch
                                    drag.maximumY: (placesList.count - 1 - placeRow.index)
                                                   * placesList.rowPitch
                                    preventStealing: true

                                    onPositionChanged: {
                                        if (!drag.active)
                                            return
                                        placesList.dragIndex = placeRow.index
                                        placesList.dropIndex = Math.max(
                                            0, Math.min(placesList.count - 1,
                                                        placeRow.index + Math.round(
                                                            placeContent.y / placesList.rowPitch)))
                                    }

                                    onReleased: {
                                        if (placesList.dragIndex !== placeRow.index)
                                            return
                                        const from = placeRow.index
                                        const to = placesList.dropIndex
                                        placeRow.justDragged = true
                                        placeContent.y = 0
                                        placesList.moveDragged(from, to)
                                    }

                                    onCanceled: {
                                        placeContent.y = 0
                                        if (placesList.dragIndex === placeRow.index) {
                                            placesList.dragIndex = -1
                                            placesList.dropIndex = -1
                                        }
                                    }

                                    onClicked: function(mouse) {
                                        if (placeRow.justDragged) {
                                            placeRow.justDragged = false
                                            return
                                        }
                                        if (mouse.button === Qt.RightButton) {
                                            const point = placeRow.mapToItem(window.contentItem,
                                                                             mouse.x, mouse.y)
                                            placeMenu.targetKey = placeRow.modelData
                                            placeMenu.targetName = placeRow.def.name
                                            placeMenu.targetPath = placeRow.placePath
                                            placeMenu.popup(window.contentItem, point)
                                        } else if (mouse.button === Qt.MiddleButton) {
                                            if (placeRow.placePath.length > 0)
                                                window.openTab(placeRow.placePath, false)
                                        } else {
                                            placeRow.activate()
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Bring hidden places back — the same escape hatch the devices
                    // list offers, so hiding one is never a one-way door.
                    Item {
                        width: placesColumn.width
                        readonly property int hidden: window.activeController
                                                      ? window.activeController.hiddenPlaceCount : 0
                        visible: hidden > 0 && !sidebar.placesCollapsed
                        height: visible ? window.sidebarRowHeight : 0

                        Text {
                            x: 12 + Math.round(CelestinaTheme.iconSm * window.sidebarIconScale) + 10
                            anchors.verticalCenter: parent.verticalCenter
                            text: "Mostrar " + parent.hidden + " ocultos"
                            color: unhidePlacesMouse.containsMouse ? CelestinaTheme.accent
                                                                   : CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontLabel * window.sidebarTextScale)
                        }

                        MouseArea {
                            id: unhidePlacesMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: if (window.activeController)
                                           window.activeController.unhideAllPlaces()
                        }
                    }

                    // ── Removable volumes (UDisks2) ──────────────────────────
                    Item {
                        width: placesColumn.width
                        readonly property var ac: window.activeController
                        readonly property int hiddenCount: ac ? ac.hiddenDeviceCount : 0
                        readonly property bool anyDevices:
                            ac && (ac.volumeNames.length > 0 || hiddenCount > 0)
                        height: anyDevices ? volumesHeaderRow.implicitHeight + 16 : 0
                        visible: anyDevices

                        SidebarChevron {

                            textScale: window.sidebarTextScale
                            x: -4
                            y: 13
                            collapsed: sidebar.devicesCollapsed
                            visible: parent.anyDevices
                        }

                        MouseArea {
                            width: parent.width
                            height: volumesHeaderRow.height + 12
                            y: 6
                            cursorShape: Qt.PointingHandCursor
                            onClicked: sidebar.devicesCollapsed = !sidebar.devicesCollapsed
                        }

                        Text {
                            id: volumesHeaderRow
                            // placesColumn.x is 8, so x:8 here → the same absolute
                            // left edge as MARCADORES (x:16), aligning the headers.
                            x: 8
                            y: 12
                            text: "DISPOSITIVOS"
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontMini * window.sidebarTextScale)
                            font.letterSpacing: 1.4
                            font.weight: CelestinaTheme.weightDemiBold
                        }

                        // Un-hide affordance — reachable even when every device is
                        // hidden (the header still shows).
                        Text {
                            anchors.verticalCenter: volumesHeaderRow.verticalCenter
                            anchors.right: parent.right
                            anchors.rightMargin: 12
                            visible: parent.hiddenCount > 0
                            text: parent.hiddenCount + " ocultos"
                            color: unhideMouse.containsMouse ? CelestinaTheme.accent
                                                             : CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontMini * window.sidebarTextScale)

                            MouseArea {
                                id: unhideMouse
                                anchors.fill: parent
                                anchors.margins: -6
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: if (window.activeController)
                                               window.activeController.unhideAllDevices()
                            }
                        }
                    }

                    Repeater {
                        // Plegar vacía el modelo: las filas dejan de existir en vez
                        // de quedarse invisibles ocupando alto en la columna.
                        model: (window.activeController && !sidebar.devicesCollapsed)
                               ? window.activeController.volumeNames : []

                        delegate: Item {
                            id: volumeRow
                            required property int index
                            required property string modelData
                            readonly property string mountPoint:
                                (window.activeController
                                 && index < window.activeController.volumeMounts.length)
                                ? window.activeController.volumeMounts[index] : ""
                            readonly property bool mounted: mountPoint.length > 0
                            readonly property bool current: mounted
                                && mountPoint === (window.activeController
                                                   ? window.activeController.currentPath : "")

                            width: placesColumn.width
                            height: window.sidebarRowHeight
                            Accessible.role: Accessible.Button
                            Accessible.name: volumeRow.modelData
                                             + (volumeRow.mounted ? ", montado" : ", sin montar")

                            Rectangle {
                                anchors.fill: parent
                                anchors.leftMargin: 2
                                anchors.rightMargin: 2
                                radius: CelestinaTheme.radiusSm
                                color: volumeRow.current
                                       ? CelestinaTheme.badgeAccentFill
                                       : volumeMouse.containsMouse
                                         ? CelestinaTheme.surfaceHover : "transparent"
                            }

                            IconImage {
                                id: volumeIcon
                                x: 12
                                anchors.verticalCenter: parent.verticalCenter
                                width: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                                height: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                                name: "drive-removable-media"
                                source: CelestinaTheme.fallbackIcon("folder")
                                // Native theme colours (no tint).
                            }

                            Text {
                                x: volumeIcon.x + volumeIcon.width + 10
                                anchors.verticalCenter: parent.verticalCenter
                                width: ejectButton.x - x - 6
                                text: volumeRow.modelData
                                color: volumeRow.current ? CelestinaTheme.accent
                                                         : CelestinaTheme.text
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: Math.round(CelestinaTheme.fontBody * window.sidebarTextScale)
                                elide: Text.ElideRight
                            }

                            // Eject (unmount) when mounted; hidden otherwise.
                            IconImage {
                                id: ejectButton
                                z: 3   // above the full-row open handler below
                                anchors.verticalCenter: parent.verticalCenter
                                anchors.right: parent.right
                                anchors.rightMargin: 10
                                width: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                                height: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                                visible: volumeRow.mounted
                                name: "media-eject"
                                source: CelestinaTheme.fallbackIcon("media-eject")
                                color: ejectMouse.containsMouse ? CelestinaTheme.accent
                                                                : CelestinaTheme.textMuted
                                opacity: ejectMouse.containsMouse ? 1.0 : 0.7
                                Accessible.role: Accessible.Button
                                Accessible.name: "Expulsar " + volumeRow.modelData

                                MouseArea {
                                    id: ejectMouse
                                    anchors.fill: parent
                                    anchors.margins: -4
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        if (window.activeController)
                                            window.activeController.unmountVolume(
                                                volumeRow.index)
                                    }
                                }
                            }

                            MouseArea {
                                id: volumeMouse
                                anchors.fill: parent
                                acceptedButtons: Qt.LeftButton | Qt.RightButton
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                // Left: open (mounting first if needed) — eject has its
                                // own zone. Right: hide this device.
                                onClicked: function(mouse) {
                                    if (!window.activeController)
                                        return
                                    if (mouse.button === Qt.RightButton) {
                                        deviceMenu.deviceName = volumeRow.modelData
                                        const point = volumeRow.mapToItem(
                                                        window.contentItem, mouse.x, mouse.y)
                                        deviceMenu.popup(window.contentItem, point)
                                    } else {
                                        window.activeController.openVolume(volumeRow.index)
                                    }
                                }
                            }
                        }
                    }
                }


                Text {
                    id: favoritesLabel
                    x: 16
                    y: placesColumn.y + placesColumn.height + 12
                    visible: sidebar.favoriteRows.length > 0
                    text: "FAVORITOS"

                    SidebarChevron {

                        textScale: window.sidebarTextScale
                        x: -12
                        anchors.verticalCenter: parent.verticalCenter
                        collapsed: sidebar.favoritesCollapsed
                    }

                    MouseArea {
                        anchors.fill: parent
                        anchors.margins: -6
                        anchors.leftMargin: -16
                        cursorShape: Qt.PointingHandCursor
                        onClicked: sidebar.favoritesCollapsed = !sidebar.favoritesCollapsed
                    }

                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontMini * window.sidebarTextScale)
                    font.letterSpacing: 1.4
                    font.weight: CelestinaTheme.weightDemiBold
                }

                ListView {
                    id: favoritesList
                    x: 8
                    y: favoritesLabel.y + (favoritesLabel.visible ? 20 : 0)
                    width: parent.width - 16
                    // Sized to its content and never scrollable: a section that
                    // scrolls inside a sidebar that also scrolls is two scrollbars
                    // arguing. The bookmarks below take whatever is left.
                    height: sidebar.favoritesCollapsed
                            ? 0 : count * (window.sidebarRowHeight + spacing)
                    interactive: false
                    visible: count > 0 && !sidebar.favoritesCollapsed
                    clip: true
                    model: sidebar.favoriteRows
                    spacing: 2
                    boundsBehavior: Flickable.StopAtBounds

                    delegate: Item {
                        id: favRow

                        required property var modelData
                        readonly property bool missing: modelData.kind === "missing"
                        readonly property bool current: !missing
                                && modelData.path === (window.activeController
                                                       ? window.activeController.currentPath : "")

                        width: favoritesList.width
                        height: window.sidebarRowHeight

                        Rectangle {
                            anchors.fill: parent
                            anchors.leftMargin: 2
                            anchors.rightMargin: 2
                            radius: CelestinaTheme.radiusSm
                            color: favRow.current
                                   ? CelestinaTheme.badgeAccentFill
                                   : favMouse.containsMouse
                                     ? CelestinaTheme.surfaceHover
                                     : "transparent"

                            Behavior on color {
                                ColorAnimation { duration: CelestinaTheme.motionFast }
                            }
                        }

                        IconImage {
                            id: favIcon
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                            height: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                            opacity: favRow.missing ? 0.45 : 1
                            name: favRow.modelData.kind === "directory"
                                  ? "folder" : "text-x-generic"
                            source: CelestinaTheme.fallbackIcon(
                                        favRow.modelData.kind === "directory" ? "folder" : "file")
                        }

                        Text {
                            x: favIcon.x + favIcon.width + 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - x - 12
                            text: favRow.modelData.name
                            color: favRow.missing ? CelestinaTheme.textMuted
                                   : favRow.current ? CelestinaTheme.accent
                                   : CelestinaTheme.text
                            // A favourite whose target is gone is struck through, not
                            // hidden: it is still a star the user set, and only they
                            // should decide to drop it.
                            font.strikeout: favRow.missing
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * window.sidebarTextScale)
                            font.weight: favRow.current ? CelestinaTheme.weightMedium
                                                        : CelestinaTheme.weightRegular
                            elide: Text.ElideMiddle
                        }

                        MouseArea {
                            id: favMouse
                            anchors.fill: parent
                            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            ToolTip.visible: containsMouse
                            ToolTip.delay: 600
                            ToolTip.text: favRow.modelData.path
                            onClicked: function(mouse) {
                                const ac = window.activeController
                                if (!ac || favRow.missing)
                                    return
                                if (mouse.button === Qt.RightButton) {
                                    const point = favRow.mapToItem(window.contentItem,
                                                                   mouse.x, mouse.y)
                                    favMenu.targetPath = favRow.modelData.path
                                    favMenu.popup(window.contentItem, point)
                                } else if (favRow.modelData.kind === "directory") {
                                    if (mouse.button === Qt.MiddleButton)
                                        window.openTab(favRow.modelData.path, false)
                                    else
                                        ac.openLocation(favRow.modelData.path)
                                } else {
                                    ac.revealPath(favRow.modelData.path)
                                }
                            }
                        }
                    }
                }

                Text {
                    id: bookmarksLabel
                    x: 16
                    // Se mide contra la *cabecera* de favoritos, no contra su
                    // lista: plegada la lista mide 0 pero la cabecera sigue ahí,
                    // y contra la lista los dos títulos se pisaban.
                    y: favoritesLabel.visible
                       ? favoritesList.y + favoritesList.height + 12
                       : favoritesLabel.y
                    text: "MARCADORES"

                    SidebarChevron {

                        textScale: window.sidebarTextScale
                        x: -12
                        anchors.verticalCenter: parent.verticalCenter
                        collapsed: sidebar.bookmarksCollapsed
                    }

                    MouseArea {
                        anchors.fill: parent
                        anchors.margins: -6
                        anchors.leftMargin: -16
                        cursorShape: Qt.PointingHandCursor
                        onClicked: sidebar.bookmarksCollapsed = !sidebar.bookmarksCollapsed
                    }

                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontMini * window.sidebarTextScale)
                    font.letterSpacing: 1.4
                    font.weight: CelestinaTheme.weightDemiBold
                }

                ListView {
                    id: bookmarksList
                    x: 8
                    y: bookmarksLabel.y + 20
                    width: parent.width - 16
                    // Mide lo que ocupa: quien desplaza es el panel entero.
                    height: sidebar.bookmarksCollapsed
                            ? 0 : count * (window.sidebarRowHeight + spacing)
                    visible: !sidebar.bookmarksCollapsed
                    clip: true
                    model: window.activeController ? window.activeController.bookmarkNames : []
                    spacing: 2
                    boundsBehavior: Flickable.StopAtBounds

                    property int editIndex: -1

                    // ── Reordering ───────────────────────────────────────────
                    // The bookmarks are the user's own list, so their order is
                    // theirs too: drag a row to move it. `dragIndex` is the row
                    // being carried, `dropIndex` where it would land; the row
                    // lifts and every other row shows the landing line.
                    property int dragIndex: -1
                    property int dropIndex: -1
                    readonly property int rowPitch: window.sidebarRowHeight + spacing

                    function moveDragged(from, to) {
                        dragIndex = -1
                        dropIndex = -1
                        // Last: the move republishes bookmarkNames, which resets
                        // this view and destroys the delegate that called us.
                        if (to >= 0 && to !== from && window.activeController)
                            window.activeController.moveBookmark(from, to)
                    }

                    delegate: Item {
                        id: bmRow

                        required property int index
                        required property string modelData

                        readonly property string path: (window.activeController
                                && index >= 0
                                && index < window.activeController.bookmarkPaths.length)
                                ? window.activeController.bookmarkPaths[index] : ""
                        readonly property bool current: path.length > 0
                                && path === (window.activeController
                                             ? window.activeController.currentPath : "")
                        readonly property bool editing: bookmarksList.editIndex === index
                        readonly property bool dragging: bookmarksList.dragIndex === index
                        // Set on release so the click that ends a drag doesn't also
                        // navigate; cleared by the click itself.
                        property bool justDragged: false

                        width: bookmarksList.width
                        height: window.sidebarRowHeight
                        // On the delegate, not on its content: z inside a row would
                        // not lift it above the neighbouring rows it overlaps.
                        z: dragging ? 2 : 0

                        // Where the carried row would land, drawn on the row it
                        // would land against — above it when moving up, below when
                        // moving down.
                        Rectangle {
                            z: 3
                            visible: bookmarksList.dragIndex >= 0
                                     && bookmarksList.dragIndex !== bmRow.index
                                     && bookmarksList.dropIndex === bmRow.index
                            x: 2
                            width: parent.width - 4
                            height: 2
                            radius: 1
                            y: bookmarksList.dropIndex > bookmarksList.dragIndex
                               ? parent.height - height : 0
                            color: CelestinaTheme.accent
                        }

                        // Everything visible lives in here so a drag can carry the
                        // row without fighting the view, which owns the delegate's
                        // own position.
                        Item {
                            id: bmContent
                            width: bmRow.width
                            height: bmRow.height
                            opacity: bmRow.dragging ? 0.9 : 1

                            Behavior on y {
                                enabled: !bmMouse.drag.active
                                NumberAnimation {
                                    duration: CelestinaTheme.motionFast
                                    easing.type: CelestinaTheme.easeStandard
                                }
                            }

                            Rectangle {
                                anchors.fill: parent
                                anchors.leftMargin: 2
                                anchors.rightMargin: 2
                                radius: CelestinaTheme.radiusSm
                                color: bmRow.dragging
                                       ? CelestinaTheme.surfaceStrong
                                       : bmRow.current
                                         ? CelestinaTheme.badgeAccentFill
                                         : bmMouse.containsMouse
                                           ? CelestinaTheme.surfaceHover
                                           : "transparent"

                                Behavior on color {
                                    ColorAnimation { duration: CelestinaTheme.motionFast }
                                }
                            }

                            IconImage {
                                id: bmIcon
                                x: 12
                                anchors.verticalCenter: parent.verticalCenter
                                width: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                                height: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                                name: "folder"
                                source: CelestinaTheme.fallbackIcon("folder")
                                // Native theme colours (no tint).
                            }

                            Text {
                                visible: !bmRow.editing
                                x: bmIcon.x + bmIcon.width + 10
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - x - 12
                                text: bmRow.modelData
                                color: bmRow.current ? CelestinaTheme.accent
                                                     : CelestinaTheme.text
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: Math.round(CelestinaTheme.fontBody * window.sidebarTextScale)
                                font.weight: bmRow.current ? CelestinaTheme.weightMedium
                                                           : CelestinaTheme.weightRegular
                                elide: Text.ElideRight
                            }

                            TextField {
                                id: bmField
                                visible: bmRow.editing
                                x: bmIcon.x + bmIcon.width + 6
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - x - 8
                                height: 26
                                text: bmRow.modelData
                                color: CelestinaTheme.text
                                selectionColor: CelestinaTheme.accentStrong
                                selectedTextColor: CelestinaTheme.text
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontLabel
                                leftPadding: 8
                                rightPadding: 8
                                background: Rectangle {
                                    radius: CelestinaTheme.radiusXs
                                    color: CelestinaTheme.inputFillFocus
                                    border.width: 1
                                    border.color: CelestinaTheme.focus
                                }
                                onVisibleChanged: if (visible) { forceActiveFocus(); selectAll() }
                                // Leave edit mode *before* renaming: the rename republishes
                                // bookmarkNames, which resets this ListView and destroys
                                // this very delegate — anything touched afterwards (the
                                // row's index, the list's id) is already gone.
                                onAccepted: {
                                    const index = bmRow.index
                                    const value = text
                                    bookmarksList.editIndex = -1
                                    if (window.activeController)
                                        window.activeController.renameBookmark(index, value)
                                }
                                onActiveFocusChanged: {
                                    if (!activeFocus && bookmarksList.editIndex === bmRow.index)
                                        bookmarksList.editIndex = -1
                                }
                                Keys.onPressed: function(event) {
                                    if (event.key === Qt.Key_Escape) {
                                        bookmarksList.editIndex = -1
                                        event.accepted = true
                                    }
                                }
                            }

                            MouseArea {
                                id: bmMouse
                                anchors.fill: parent
                                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                // Carry the row itself, bounded to the list so it can
                                // never be dragged out of its own section. The list is
                                // a Flickable, so hold the grab once the drag starts or
                                // it would scroll instead of reorder.
                                drag.target: bmRow.editing ? null : bmContent
                                drag.axis: Drag.YAxis
                                drag.smoothed: false
                                drag.threshold: 6
                                drag.minimumY: -bmRow.index * bookmarksList.rowPitch
                                drag.maximumY: (bookmarksList.count - 1 - bmRow.index)
                                               * bookmarksList.rowPitch
                                preventStealing: true

                                onPositionChanged: {
                                    if (!drag.active)
                                        return
                                    bookmarksList.dragIndex = bmRow.index
                                    bookmarksList.dropIndex = Math.max(
                                        0, Math.min(bookmarksList.count - 1,
                                                    bmRow.index + Math.round(
                                                        bmContent.y / bookmarksList.rowPitch)))
                                }

                                onReleased: {
                                    if (bookmarksList.dragIndex !== bmRow.index)
                                        return
                                    const from = bmRow.index
                                    const to = bookmarksList.dropIndex
                                    bmRow.justDragged = true
                                    bmContent.y = 0
                                    bookmarksList.moveDragged(from, to)
                                }

                                onCanceled: {
                                    bmContent.y = 0
                                    if (bookmarksList.dragIndex === bmRow.index) {
                                        bookmarksList.dragIndex = -1
                                        bookmarksList.dropIndex = -1
                                    }
                                }

                                onClicked: function(mouse) {
                                    // The release that ends a reorder is not a click.
                                    if (bmRow.justDragged) {
                                        bmRow.justDragged = false
                                        return
                                    }
                                    if (mouse.button === Qt.MiddleButton) {
                                        window.openTab(bmRow.path, false)
                                    } else if (mouse.button === Qt.RightButton) {
                                        const point = bmRow.mapToItem(window.contentItem,
                                                                      mouse.x, mouse.y)
                                        bmMenu.targetIndex = bmRow.index
                                        bmMenu.popup(window.contentItem, point)
                                    } else if (window.activeController) {
                                        window.activeController.openLocation(bmRow.path)
                                    }
                                }
                                onDoubleClicked: bookmarksList.editIndex = bmRow.index
                            }
                        }
                    }
                }
            }

        }

        // A separate box below the sidebar (its own panel, not nested inside it)
        // showing item info: the folder's count + total size when nothing is
        // selected, the selected item's name + kind · size · date for one, or the
        // count for a multi-selection.
        Rectangle {
            id: sidebarInfo
            x: sidebar.x
            width: sidebar.width
            height: Math.round(84 * window.sidebarTextScale)
            y: parent.height - height - 18
            visible: sidebar.visible
            radius: CelestinaTheme.radiusLg
            color: CelestinaTheme.surface
            border.width: 1
            border.color: CelestinaTheme.border

            readonly property var ac: window.activeController
            readonly property int selCount: ac ? ac.selectionCount : 0
            readonly property int count: ac ? ac.entryNames.length : 0
            // Re-evaluated when the list changes (entryNames) so the index stays
            // valid across sort/filter.
            readonly property int selIdx: {
                var _ = ac ? ac.entryNames.length : 0
                return (ac && selCount === 1 && ac.selectedToken.length > 0)
                       ? ac.indexForToken(ac.selectedToken) : -1
            }

            readonly property string header: selCount > 1 ? "SELECCIÓN"
                                             : selIdx >= 0 ? "ELEMENTO" : "CARPETA"
            readonly property string primary: selCount > 1
                    ? selCount + " seleccionados"
                    : selIdx >= 0 ? ac.entryNames[selIdx]
                    : count + (count === 1 ? " elemento" : " elementos")
            readonly property string secondary: selCount > 1 ? ""
                    : selIdx >= 0 ? ac.entryDetail(selIdx)
                    : (ac && ac.folderSize.length > 0 ? "Total " + ac.folderSize : "")

            Column {
                x: 18
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 34
                spacing: 4

                Text {
                    text: sidebarInfo.header
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontLabel * window.sidebarTextScale)
                    font.letterSpacing: 1.4
                    font.weight: CelestinaTheme.weightDemiBold
                }

                Text {
                    width: parent.width
                    text: sidebarInfo.primary
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontCallout * window.sidebarTextScale)
                    font.weight: CelestinaTheme.weightMedium
                    elide: Text.ElideMiddle
                }

                Text {
                    width: parent.width
                    visible: sidebarInfo.secondary.length > 0
                    text: sidebarInfo.secondary
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontLabel * window.sidebarTextScale)
                    elide: Text.ElideRight
                }
            }
        }

        // ── Documents: one per tab, only the active one visible ──────────
        Item {
            id: documentRegion
            x: sidebar.visible ? sidebar.x + sidebar.width + 14 : 20
            y: 18
            width: parent.width - x - 20
            height: parent.height - y - 20

            Repeater {
                id: tabRepeater
                model: tabsModel

                delegate: Item {
                    id: tabHolder

                    required property int index
                    required property string initialPath

                    anchors.fill: parent
                    visible: index === window.currentTabIndex
                    readonly property var docController: doc.tabController

                    FolderView {
                        id: doc
                        anchors.fill: parent
                        active: tabHolder.visible
                        ghost: dragGhost
                        overlayParent: window.contentItem
                        hostWindow: window

                        onRequestNewTab: function(path, foreground) {
                            window.openTab(path, foreground)
                        }

                        // A tab scans when it is first *shown*, not when it is
                        // created. A restored session of five tabs used to fire
                        // five directory scans at once — four of them for
                        // folders nobody was looking at — so a session on big
                        // folders paid for all of them before the first frame.
                        property bool started: false

                        function startIfNeeded() {
                            if (started || !tabHolder.visible)
                                return
                            started = true
                            if (tabHolder.initialPath.length > 0)
                                doc.tabController.startAt(tabHolder.initialPath)
                            else
                                doc.tabController.start()
                        }

                        Component.onCompleted: startIfNeeded()
                        onActiveChanged: startIfNeeded()

                        Connections {
                            target: doc.tabController
                            function onCurrentPathChanged() {
                                tabsModel.setProperty(
                                    tabHolder.index, "title",
                                    window.tabTitle(doc.tabController.currentPath))
                                sessionSaver.restart()
                            }
                        }
                    }
                }
            }
        }

        Connections {
            target: tabRepeater
            function onItemAdded() { window.tabsRevision++; sessionSaver.restart() }
            function onItemRemoved() { window.tabsRevision++; sessionSaver.restart() }
        }
    }

    GlassContextMenu {
        id: bmMenu
        backdropSource: contentLayer

        property int targetIndex: -1

        GlassMenuItem {
            text: "Renombrar"
            onTriggered: bookmarksList.editIndex = bmMenu.targetIndex
        }

        // The same reorder the drag does, reachable from the keyboard and a
        // screen reader — a drag is not an accessible way to be the only one.
        GlassMenuItem {
            text: "Subir"
            enabled: bmMenu.targetIndex > 0
            icon.name: "go-up"
            icon.source: CelestinaTheme.fallbackIcon("go-up")
            onTriggered: {
                if (window.activeController)
                    window.activeController.moveBookmark(bmMenu.targetIndex,
                                                         bmMenu.targetIndex - 1)
            }
        }

        GlassMenuItem {
            text: "Bajar"
            enabled: bmMenu.targetIndex >= 0
                     && bmMenu.targetIndex < bookmarksList.count - 1
            icon.name: "go-down"
            icon.source: CelestinaTheme.fallbackIcon("go-up")
            onTriggered: {
                if (window.activeController)
                    window.activeController.moveBookmark(bmMenu.targetIndex,
                                                         bmMenu.targetIndex + 1)
            }
        }

        GlassMenuItem {
            text: "Quitar de marcadores"
            onTriggered: {
                if (window.activeController)
                    window.activeController.removeBookmark(bmMenu.targetIndex)
            }
        }
    }

    // Right-click menu for a sidebar place.
    GlassContextMenu {
        id: placeMenu
        backdropSource: contentLayer

        property string targetKey: ""
        property string targetName: ""
        property string targetPath: ""

        GlassMenuItem {
            text: "Abrir en pestaña nueva"
            visible: placeMenu.targetPath.length > 0
            height: visible ? implicitHeight : 0
            icon.name: "tab-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: window.openTab(placeMenu.targetPath, true)
        }

        GlassMenuItem {
            text: "Ocultar «" + placeMenu.targetName + "»"
            icon.name: "list-remove"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: {
                if (window.activeController)
                    window.activeController.hidePlace(placeMenu.targetKey)
            }
        }

        GlassMenuItem {
            text: "Mostrar lugares ocultos"
            visible: window.activeController
                     && window.activeController.hiddenPlaceCount > 0
            height: visible ? implicitHeight : 0
            onTriggered: {
                if (window.activeController)
                    window.activeController.unhideAllPlaces()
            }
        }
    }

    // Right-click menu for a row in the "Favoritos" list.
    GlassContextMenu {
        id: favMenu
        backdropSource: contentLayer

        property string targetPath: ""

        GlassMenuItem {
            text: "Abrir en pestaña nueva"
            icon.name: "tab-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: window.openTab(favMenu.targetPath, true)
        }

        GlassMenuItem {
            text: "Mostrar en su carpeta"
            icon.name: "folder-open"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: {
                if (window.activeController)
                    window.activeController.revealPath(favMenu.targetPath)
            }
        }

        GlassMenuItem {
            text: "Quitar de favoritos"
            icon.source: CelestinaTheme.fallbackIcon("star")
            onTriggered: {
                if (window.activeController)
                    window.activeController.toggleFavorite(favMenu.targetPath)
            }
        }
    }

    // Right-click menu for a device in the "Dispositivos" list.
    GlassContextMenu {
        id: deviceMenu
        backdropSource: contentLayer

        property string deviceName: ""

        GlassMenuItem {
            text: "Ocultar dispositivo"
            icon.name: "list-remove"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: {
                if (window.activeController)
                    window.activeController.hideDevice(deviceMenu.deviceName)
            }
        }

        GlassMenuItem {
            text: "Mostrar dispositivos ocultos"
            visible: window.activeController
                     && window.activeController.hiddenDeviceCount > 0
            height: visible ? implicitHeight : 0
            onTriggered: {
                if (window.activeController)
                    window.activeController.unhideAllDevices()
            }
        }
    }

    Component.onCompleted: {
        window.width = sessionStore.savedWindowWidth()
        window.height = sessionStore.savedWindowHeight()

        // A launch that names a folder is about that folder: the saved session
        // does not talk over it, it just does not reopen this time.
        const saved = sessionStore.launchPathGiven() ? [] : sessionStore.savedTabs()
        if (saved.length === 0) {
            tabsModel.append({ initialPath: "", title: "…" })
            window.currentTabIndex = 0
        } else {
            for (var i = 0; i < saved.length; i++)
                tabsModel.append({ initialPath: saved[i],
                                   title: window.tabTitle(saved[i]) })
            window.currentTabIndex = Math.max(
                0, Math.min(tabsModel.count - 1, sessionStore.savedActiveTab()))
        }
        // Activated by the portal to serve a file chooser: this process has no
        // main window, only the pickers it is asked for.
        window.visible = !portalService.portalMode()
    }
}
