import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.siderita 1.0
import org.celestina.siderita.internal 1.0

ApplicationWindow {
    id: window

    required property bool reducedMotion

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
    readonly property var activeDocument: {
        tabsRevision // re-evaluate when tabs are created or destroyed
        const holder = tabRepeater.itemAt(currentTabIndex)
        return holder ? holder.docView : null
    }
    readonly property bool lowerSurfaceBlocked:
            activeDocument && activeDocument.navigationBlocked

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

    function tabTitle(controller, p) {
        if (!p || p.length === 0)
            return "…"
        if (controller)
            return controller.displayLocationName(p)
        if (p === "/")
            return "/"
        const s = p.replace(/\/+$/, "")
        const i = s.lastIndexOf("/")
        return i >= 0 ? s.substring(i + 1) : s
    }

    function openTab(path, foreground) {
        const initial = (path === undefined || path === null) ? "" : path
        tabsModel.append({
            initialPath: initial,
            title: window.tabTitle(window.activeController, initial)
        })
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

    // ── Activation: which application opens a file ───────────────────────
    // Double-click and Enter route through here. Whether a file is text is
    // decided by its *bytes*, so a `notas.mp3` that is really text opens in
    // Grafita and a binary named `.txt` does not — which is the whole reason
    // this is not left to the desktop's MIME lookup.
    //
    // One activator per window rather than per tab: the question is about a
    // file, not about a folder view, and a single owner means a single bounded
    // worker. Delegates reach it through `Window.window`, which is what lets
    // this land without the folder view having to hand anything down.
    GrafitaEditor {
        id: textActivator

        property var pendingController: null
        property string pendingToken: ""

        onLaunchDecided: function(path, editable) {
            const controller = textActivator.pendingController
            const token = textActivator.pendingToken
            textActivator.pendingController = null
            textActivator.pendingToken = ""
            // Text goes to Grafita; anything else — and a Grafita that is not
            // installed — falls back to the desktop's own handler, so a failed
            // launch still opens the file.
            if (editable && textActivator.launchStandalone(path))
                return
            if (controller)
                controller.activateToken(token)
        }
    }

    // Called by every activation site instead of `controller.activateToken`.
    // Directories and unknown entries never take the detour: a folder has no
    // bytes to classify, and asking would be a wasted read.
    function activateEntry(controller, token) {
        const index = controller.indexForToken(token)
        if (index < 0 || controller.entryKind(index) === "directory") {
            controller.activateToken(token)
            return
        }
        const path = controller.entryPath(index)
        if (path.length === 0) {
            controller.activateToken(token)
            return
        }
        textActivator.pendingController = controller
        textActivator.pendingToken = token
        textActivator.requestLaunch(path)
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

        // Activated to be the file chooser, but another process already is one.
        // There is nothing for this one to do and no window to do it in, so it
        // leaves instead of parking a few hundred megabytes until logout. A
        // Siderita the user opened stays open: it simply does not own the
        // backend this time.
        onBackendUnavailable: function(reason) {
            if (portalService.portalMode()) {
                console.warn("Siderita: sin backend de portal (" + reason + "); saliendo")
                Qt.quit()
            }
        }

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
        enabled: !window.lowerSurfaceBlocked
        onActivated: window.openTab(window.activeController
                                    ? window.activeController.currentPath : "", true)
    }

    Shortcut {
        sequence: "Ctrl+W"
        enabled: tabsModel.count > 1 && !window.lowerSurfaceBlocked
        onActivated: window.closeTab(window.currentTabIndex)
    }

    Shortcut {
        sequence: "Ctrl+Tab"
        enabled: tabsModel.count > 1 && !window.lowerSurfaceBlocked
        onActivated: window.cycleTab(1)
    }

    Shortcut {
        sequence: "Ctrl+Shift+Tab"
        enabled: tabsModel.count > 1 && !window.lowerSurfaceBlocked
        onActivated: window.cycleTab(-1)
    }

    Shortcut {
        sequence: "Ctrl+PgDown"
        enabled: tabsModel.count > 1 && !window.lowerSurfaceBlocked
        onActivated: window.cycleTab(1)
    }

    Shortcut {
        sequence: "Ctrl+PgUp"
        enabled: tabsModel.count > 1 && !window.lowerSurfaceBlocked
        onActivated: window.cycleTab(-1)
    }

    Item {
        id: contentLayer
        anchors.fill: parent

        CelestinaBackdrop {
            anchors.fill: parent
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

        // FolderView owns its dialogs, so their shared modal layer only spans
        // the document region. Complete that same modal boundary over the
        // window-owned sidebar: dim it and consume every pointer button while
        // the active document is opening, showing or fading out a dialog.
        Rectangle {
            id: sidebarModalShield
            x: 0
            y: 0
            width: documentRegion.x
            height: parent.height
            visible: window.activeDocument
                     && window.activeDocument.modalBlocked
            color: CelestinaTheme.scrim
            Accessible.ignored: true

            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.AllButtons
                hoverEnabled: true
                preventStealing: true
                onWheel: function(wheel) { wheel.accepted = true }
            }
        }

        // ── Documents: one per tab, only the active one visible ──────────
        Item {
            id: documentRegion
            x: sidebarPanel.panelVisible ? sidebarPanel.rightEdge + 14 : 14
            y: 14
            width: parent.width - x - 14
            height: parent.height - y - 14

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
                    readonly property var docView: doc

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
                            function refreshTitle() {
                                tabsModel.setProperty(
                                    tabHolder.index, "title",
                                    window.tabTitle(doc.tabController,
                                                    doc.tabController.currentPath))
                            }
                            function onCurrentPathChanged() { refreshTitle() }
                            function onPhoneNamesChanged() { refreshTitle() }
                            function onPhoneMountsChanged() { refreshTitle() }
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

    // Back/Forward are window actions: they work over the sidebar, the file
    // view and floating chrome alike. Popup.Item overlays remain above this
    // content layer, and local modal state disables it explicitly.
    HistoryMouseArea {
        anchors.fill: parent
        z: 1000
        blocked: !window.activeDocument
                 || window.activeDocument.navigationBlocked
        canGoBack: window.activeDocument
                   ? window.activeDocument.canGoBackOrLeave : false
        canGoForward: window.activeController
                      ? window.activeController.canGoForward
                        && !window.activeController.loading
                      : false
        onBackRequested: window.activeDocument.goBackOrLeave()
        onForwardRequested: window.activeController.goForward()
    }


    Component.onCompleted: {
        CelestinaTheme.reducedMotion = reducedMotion
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
