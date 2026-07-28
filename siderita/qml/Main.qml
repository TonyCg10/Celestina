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
    // first tab exists — so the window never flashes at the default size and
    // then jumps.
    visible: false
    color: CelestinaTheme.canvas
    title: "Siderita"

    // ── Session ──────────────────────────────────────────────────────────
    // A controller that owns no tab, used only to read and write what belongs
    // to the window rather than to any one tab: its size and the set of open
    // tabs. It is never started, so it runs no scan and holds no watch — it is
    // the settings store the window needs before the first tab exists.
    SideritaController {
        id: sessionStore
    }

    // Se posa antes de escribir: el tamaño cambia mucho mientras se arrastra la
    // ventana, así que se guarda una sola vez al parar.
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
    // Los tipos propios de Siderita (pastillas, botones, filas, la vista de
    // carpeta y el panel lateral) viven cada uno en su fichero, junto a éste.

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
                window.activeController.loadPhones()
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
            id: entryDragGhost
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
                entryDragGhost.path = entryPath
                entryDragGhost.label = entryLabel
                entryDragGhost.isDir = entryIsDir
                entryDragGhost.Drag.keys = entryIsDir
                    ? ["siderita-entry", "siderita-bookmark"]
                    : ["siderita-entry"]
                entryDragGhost.Drag.mimeData = {
                    "text/uri-list": "file://" + encodeURI(entryPath) + "\r\n"
                }
            }
        }

        // El panel lateral, con su caja de información y sus menús.
        Sidebar {
            id: sidebarPanel
            anchors.fill: parent
            hostWindow: window
            overlayParent: window.contentItem
            backdrop: contentLayer
            dragGhost: entryDragGhost
        }

        // ── Documents: one per tab, only the active one visible ──────────
        Item {
            id: documentRegion
            x: sidebarPanel.panelVisible ? sidebarPanel.rightEdge + 14 : 20
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
                        ghost: entryDragGhost
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
                            }
                        }
                    }
                }
            }
        }

        Connections {
            target: tabRepeater
            function onItemAdded() { window.tabsRevision++ }
            function onItemRemoved() { window.tabsRevision++ }
        }
    }


    Component.onCompleted: {
        window.width = sessionStore.savedWindowWidth()
        window.height = sessionStore.savedWindowHeight()

        // Siempre abre en Inicio: una ventana nueva no vuelve a donde estabas la
        // última vez. La pestaña vacía deja que el controlador resuelva Inicio, o
        // la carpeta que se pase por línea de órdenes —esa sí manda—. El historial
        // de atrás/adelante de los botones del ratón es de la sesión y se
        // construye desde aquí, así que abrir en Inicio no lo rompe.
        tabsModel.append({ initialPath: "", title: "…" })
        window.currentTabIndex = 0
        // Activated by the portal to serve a file chooser: this process has no
        // main window, only the pickers it is asked for.
        window.visible = !portalService.portalMode()
    }
}
