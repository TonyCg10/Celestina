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
    visible: true
    color: CelestinaTheme.canvas
    title: "Siderita · Iteración 1"

    // ── Tabs ─────────────────────────────────────────────────────────────
    // Each tab is an independent Document with its own SideritaController
    // (history, scan worker, selection). The window chrome — sidebar and tab
    // strip — binds to the active tab's controller via `activeController`.
    property int currentTabIndex: 0
    // Bumped whenever the tab Repeater adds/removes an item, so the
    // `activeController` binding re-resolves itemAt() after the delegate exists.
    property int tabsRevision: 0
    // The Trash ("Papelera") overlay is opened from the window-scope sidebar but
    // lives per-tab; this flag lets the sidebar drive the active tab's overlay.
    property bool trashViewOpen: false
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

    // A freshly-activated tab may have been created before a bookmark was added
    // in another tab; re-read the shared file so its sidebar is truthful.
    onCurrentTabIndexChanged: {
        if (window.activeController)
            window.activeController.reloadBookmarks()
    }

    // Exposed as a property (not merely an id) so each per-tab Document can
    // reach it through `tabHost`; a plain child id is invisible as
    // `window.tabsModel`.
    property alias tabsModel: tabsModelData

    ListModel {
        id: tabsModelData
    }

    component NavButton: ToolButton {
        id: control

        required property string iconName
        required property string fallbackIcon
        required property string helpText

        implicitWidth: CelestinaTheme.controlHeight
        implicitHeight: CelestinaTheme.controlHeight
        hoverEnabled: true
        ToolTip.visible: hovered
        ToolTip.text: helpText
        ToolTip.delay: 550
        Accessible.name: helpText
        display: AbstractButton.IconOnly
        icon.name: iconName
        icon.source: CelestinaTheme.fallbackIcon(fallbackIcon)
        icon.width: CelestinaTheme.iconSm
        icon.height: CelestinaTheme.iconSm
        icon.color: control.enabled
                    ? CelestinaTheme.text
                    : CelestinaTheme.textMuted

        background: Rectangle {
            radius: CelestinaTheme.radiusSm
            color: control.hovered
                   ? CelestinaTheme.surfaceHover
                   : CelestinaTheme.surface
            border.width: control.activeFocus ? 1 : 0
            border.color: CelestinaTheme.focus

            Behavior on color {
                ColorAnimation {
                    duration: CelestinaTheme.motionFast
                }
            }
        }
    }

    // Themed push button for dialogs/overlays (the default QtQuick Basic Button
    // is unstyled). `primary` fills with the accent; otherwise a control-fill
    // pill with a border.
    component PillButton: Button {
        id: pill

        property bool primary: false
        // A danger-tinted outline variant for irreversible actions (empty Trash).
        property bool destructive: false

        hoverEnabled: true
        implicitHeight: 30
        leftPadding: 14
        rightPadding: 14
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontLabel
        font.weight: CelestinaTheme.weightMedium

        contentItem: Text {
            text: pill.text
            font: pill.font
            color: !pill.enabled
                   ? CelestinaTheme.textMuted
                   : pill.primary ? CelestinaTheme.canvas
                   : pill.destructive ? CelestinaTheme.dangerText
                   : CelestinaTheme.text
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }

        background: Rectangle {
            radius: CelestinaTheme.radiusSm
            opacity: pill.enabled ? 1 : 0.5
            color: {
                if (!pill.enabled)
                    return CelestinaTheme.controlFill
                if (pill.primary)
                    return pill.down ? Qt.darker(CelestinaTheme.accent, 1.18)
                         : pill.hovered ? Qt.darker(CelestinaTheme.accent, 1.08)
                         : CelestinaTheme.accent
                if (pill.destructive)
                    return pill.down ? CelestinaTheme.danger
                         : pill.hovered ? CelestinaTheme.dangerBorder
                         : CelestinaTheme.dangerFill
                return pill.down ? CelestinaTheme.surfaceStrong
                     : pill.hovered ? CelestinaTheme.surfaceHover
                     : CelestinaTheme.controlFill
            }
            border.width: pill.primary ? 0 : 1
            border.color: pill.activeFocus ? CelestinaTheme.focus
                          : pill.destructive ? CelestinaTheme.dangerBorder
                          : CelestinaTheme.border

            Behavior on color {
                ColorAnimation { duration: CelestinaTheme.motionFast }
            }
        }
    }

    // A labelled zoom row for the sizing popup: caption · slider · percent.
    // The consumer binds `value` to a scale and updates it in `onMoved`.
    component SizeRow: Item {
        id: sizeRow
        property string label: ""
        property alias value: sizeSlider.value
        // Most scales cap at 2.0 (100 %); content icons may go to 3.0 (150 %).
        property real maxValue: 2.0
        signal moved(real v)

        implicitWidth: 252
        implicitHeight: 30

        Text {
            id: sizeRowLabel
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: 94
            text: sizeRow.label
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontLabel
            elide: Text.ElideRight
        }

        Slider {
            id: sizeSlider
            anchors.left: sizeRowLabel.right
            anchors.right: sizeRowValue.left
            anchors.rightMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            // The factor reads to the user as a fraction of 2.0 — 10 %–100 %
            // (or up to 150 % where maxValue is raised).
            from: 0.2
            to: sizeRow.maxValue
            stepSize: 0.1
            onMoved: sizeRow.moved(value)

            background: Rectangle {
                x: sizeSlider.leftPadding
                y: sizeSlider.topPadding + sizeSlider.availableHeight / 2 - height / 2
                width: sizeSlider.availableWidth
                height: 4
                radius: 2
                color: CelestinaTheme.controlFill

                Rectangle {
                    width: sizeSlider.visualPosition * parent.width
                    height: parent.height
                    radius: 2
                    color: CelestinaTheme.accent
                }
            }

            handle: Rectangle {
                x: sizeSlider.leftPadding
                   + sizeSlider.visualPosition * (sizeSlider.availableWidth - width)
                y: sizeSlider.topPadding + sizeSlider.availableHeight / 2 - height / 2
                width: 15
                height: 15
                radius: 7.5
                color: sizeSlider.pressed ? CelestinaTheme.accent : CelestinaTheme.text
                border.width: 1
                border.color: CelestinaTheme.borderStrong
            }
        }

        Text {
            id: sizeRowValue
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: 38
            horizontalAlignment: Text.AlignRight
            text: Math.round(sizeSlider.value / 2.0 * 100) + "%"
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }
    }

    // One label : value line in the properties panel; hides itself when empty.
    component PropRow: Item {
        id: propRow
        property string label: ""
        property string value: ""
        visible: value.length > 0
        implicitHeight: visible ? Math.max(propValue.implicitHeight, 18) + 7 : 0
        height: implicitHeight

        Text {
            id: propLabel
            y: 3
            width: 104
            text: propRow.label
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontLabel
        }
        Text {
            id: propValue
            anchors.left: propLabel.right
            anchors.leftMargin: 8
            y: 3
            width: propRow.width - propLabel.width - 8
            text: propRow.value
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontLabel
            wrapMode: Text.WrapAnywhere
        }
    }

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

    // ── Document ─────────────────────────────────────────────────────────
    // One independent folder view: breadcrumb + search pills, the list/grid,
    // multi-selection, the entry/folder/sort context menus, and the per-tab
    // navigation shortcuts. It owns its own controller (id kept as `controller`
    // so the body reads exactly like the single-view original). Only outward
    // references are parameterized: `ghost` (the shared drag ghost) and
    // `overlayParent` (window.contentItem, where popups are placed).
    component Document: Item {
        id: root

        property Item ghost
        property Item overlayParent
        property var tabHost          // window: tab model + open/close/select API
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

        // Per-tab shortcuts: only the active (visible) tab responds, so the
        // same sequence across tabs is never ambiguous.
        Shortcut {
            sequence: "Alt+Left"
            enabled: root.active && controller.canGoBack && !controller.loading
            onActivated: controller.goBack()
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
            enabled: root.active
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
            enabled: root.active
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
                    // A search in progress (either the live filter or the
                    // recursive results) bows out first — clearing the field and
                    // hits and staying put — before Back walks history.
                    if (searchField.text.length > 0 || controller.searchActive)
                        root.clearSearch()
                    else if (controller.canGoBack)
                        controller.goBack()
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
            // (window.contentIconScale / window.contentTextScale).
            Component.onCompleted: {
                viewMode = controller.savedViewMode()
                rebuildFolderTypeIcons()
            }
            function persist() { controller.saveViewMode(viewMode) }

            // The content row/cell (the "selection square") sizes to fit
            // whichever is taller — the icon or the independently-scaled text —
            // so the two size sliders never clip one another.
            readonly property int listRowHeight: Math.max(
                Math.round(CelestinaTheme.glyphTile * window.contentIconScale),
                Math.round((CelestinaTheme.fontBody + CelestinaTheme.fontCaption)
                           * 1.35 * window.contentTextScale)) + 16
            readonly property int gridCellWidth: Math.round(
                104 * Math.max(window.contentIconScale, window.contentTextScale))
            readonly property int gridCellHeight:
                Math.round(72 * window.contentIconScale) + 8
                + Math.round(CelestinaTheme.fontCaption * 2.9 * window.contentTextScale) + 20

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
            // The themed icon a non-thumbnailed entry shows — a media-type icon
            // (video/audio/image), a type-specific folder, else generic.
            function mediaIconName(kind, media, path) {
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
            Item {
                id: bottomBar
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 54

                Rectangle {
                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    height: 1
                    color: CelestinaTheme.border
                }
            }

            // Bottom-bar controls flow left-to-right and size to their (scaled)
            // content, so a larger interface text never clips or overlaps them.
            RowLayout {
                id: bottomControls
                x: 16
                anchors.verticalCenter: bottomBar.verticalCenter
                spacing: 10

                Rectangle {
                    id: hiddenToggle
                    Layout.preferredWidth: hiddenLabel.implicitWidth + 22
                    Layout.preferredHeight: 26
                    radius: CelestinaTheme.radiusXs
                    color: controller.showHidden
                           ? CelestinaTheme.badgeAccentFill
                           : hiddenMouse.containsMouse
                             ? CelestinaTheme.surfaceHover
                             : CelestinaTheme.controlFill

                    Behavior on color {
                        ColorAnimation { duration: CelestinaTheme.motionFast }
                    }

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
                        font.pixelSize: Math.round(CelestinaTheme.fontMini * window.interfaceTextScale)
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
                        font.pixelSize: Math.round(CelestinaTheme.fontCaption * window.interfaceTextScale)
                        font.weight: CelestinaTheme.weightMedium
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
                    }

                    background: Rectangle {
                        radius: CelestinaTheme.radiusSm
                        color: sortButton.hovered
                               ? CelestinaTheme.surfaceHover
                               : CelestinaTheme.controlFill
                        border.width: sortButton.activeFocus ? 1 : 0
                        border.color: CelestinaTheme.focus

                        Behavior on color {
                            ColorAnimation { duration: CelestinaTheme.motionFast }
                        }
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

                // Segmented view switch: Lista / Cuadrícula / Detalles, the
                // active mode highlighted.
                Rectangle {
                    id: viewSeg
                    Layout.preferredHeight: 34
                    Layout.preferredWidth: viewSegRow.implicitWidth + 6
                    radius: CelestinaTheme.radiusSm
                    color: CelestinaTheme.controlFill

                    Row {
                        id: viewSegRow
                        anchors.centerIn: parent
                        spacing: 3

                        Repeater {
                            model: [
                                { mode: "list", label: "Lista" },
                                { mode: "grid", label: "Cuadrícula" },
                                { mode: "details", label: "Detalles" }
                            ]

                            delegate: Rectangle {
                                id: seg
                                required property var modelData
                                readonly property bool active: mainPanel.viewMode === modelData.mode
                                width: segLabel.implicitWidth + 20
                                height: 28
                                radius: CelestinaTheme.radiusXs
                                color: seg.active ? CelestinaTheme.surfaceSelected
                                       : segMouse.containsMouse ? CelestinaTheme.surfaceHover
                                       : "transparent"

                                Behavior on color {
                                    ColorAnimation { duration: CelestinaTheme.motionFast }
                                }

                                Accessible.role: Accessible.RadioButton
                                Accessible.name: "Vista " + seg.modelData.label
                                Accessible.checked: seg.active

                                Text {
                                    id: segLabel
                                    anchors.centerIn: parent
                                    text: seg.modelData.label
                                    color: seg.active ? CelestinaTheme.text : CelestinaTheme.textMuted
                                    font.family: CelestinaTheme.sansFamily
                                    font.pixelSize: Math.round(CelestinaTheme.fontCaption * window.interfaceTextScale)
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
                height: parent.height - 68
                // The list backs three modes: plain "list", the "details"
                // columns (same rows, a different delegate body), and search
                // (which always uses the sectioned list — a grid can't carry
                // section headers).
                visible: mainPanel.viewMode === "list"
                         || mainPanel.viewMode === "details"
                         || controller.searchActive
                readonly property bool detailsMode: mainPanel.viewMode === "details"
                                                    && !controller.searchActive
                // Column widths shared by the details rows and their header
                // (name fills the rest); they track the content text scale.
                readonly property int colSizeW: Math.round(92 * window.contentTextScale)
                readonly property int colDateW: Math.round(150 * window.contentTextScale)
                readonly property int colTypeW: Math.round(96 * window.contentTextScale)
                // Where the name column starts — past the row's icon glyph — so
                // the header lines up with the rows.
                readonly property int detailsNameX: 14
                        + Math.round(CelestinaTheme.glyphTile * window.contentIconScale) + 12
                model: entryModel
                clip: true
                spacing: 2
                reuseItems: true
                cacheBuffer: 420
                topMargin: 62 + (tabBar.visible ? tabBar.height + 8 : 0)
                         + (searchBar.visible ? searchBar.height + 8 : 0)
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
                            ? Math.round(CelestinaTheme.fontMini * window.contentTextScale) + 22
                            : 0
                    visible: sectionHeader.section.length > 0

                    Text {
                        x: 14
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: 6
                        text: sectionHeader.section.toUpperCase()
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontMini * window.contentTextScale)
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
                        anchors.fill: parent
                        enabled: row.isDirectory

                        onEntered: function(drag) {
                            if (!drag.hasUrls && !mainPanel.isEntryDrag(drag))
                                drag.accepted = false
                        }
                        onDropped: function(drop) {
                            mainPanel.dropOnto(row.path, drop)
                            drop.accept()
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
                        width: Math.round(CelestinaTheme.glyphTile * window.contentIconScale)
                        height: Math.round(CelestinaTheme.glyphTile * window.contentIconScale)
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
                            width: Math.round(CelestinaTheme.iconMd * window.contentIconScale)
                            height: Math.round(CelestinaTheme.iconMd * window.contentIconScale)
                            name: mainPanel.mediaIconName(row.kind, kindGlyph.media, row.path)
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
                            visible: ready
                            source: kindGlyph.media !== ""
                                    ? "image://thumb/" + encodeURIComponent(row.path) : ""
                            sourceSize.width: 256
                            sourceSize.height: 256
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                            cache: true
                            smooth: true
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
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * window.contentTextScale)
                            font.weight: CelestinaTheme.weightMedium
                            font.italic: row.cut
                            elide: Text.ElideMiddle
                        }

                        Text {
                            width: parent.width
                            text: row.subtitle
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontCaption * window.contentTextScale)
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
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * window.contentTextScale)
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
                            font.pixelSize: Math.round(CelestinaTheme.fontCaption * window.contentTextScale)
                            elide: Text.ElideRight
                        }
                        Text {
                            Layout.preferredWidth: fileList.colDateW
                            text: row.dateText
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontCaption * window.contentTextScale)
                            elide: Text.ElideRight
                        }
                        Text {
                            Layout.preferredWidth: fileList.colTypeW
                            text: row.kind === "directory" ? "Carpeta"
                                  : row.kind === "symlink" ? "Enlace" : "Archivo"
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontCaption * window.contentTextScale)
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
                height: parent.height - 68
                visible: mainPanel.viewMode === "grid" && !controller.searchActive
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
                        anchors.fill: parent
                        anchors.margins: 5
                        enabled: cell.isDirectory

                        onEntered: function(drag) {
                            if (!drag.hasUrls && !mainPanel.isEntryDrag(drag))
                                drag.accepted = false
                        }
                        onDropped: function(drop) {
                            mainPanel.dropOnto(cell.path, drop)
                            drop.accept()
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
                            width: Math.round(72 * window.contentIconScale)
                            height: Math.round(72 * window.contentIconScale)
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
                                width: Math.round(54 * window.contentIconScale)
                                height: Math.round(54 * window.contentIconScale)
                                name: mainPanel.mediaIconName(cell.kind, cellGlyph.media, cell.path)
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
                                visible: ready
                                source: cellGlyph.media !== ""
                                        ? "image://thumb/" + encodeURIComponent(cell.path) : ""
                                sourceSize.width: 256
                                sourceSize.height: 256
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                                cache: true
                                smooth: true
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
                            font.pixelSize: Math.round(CelestinaTheme.fontCaption * window.contentTextScale)
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
                    font.pixelSize: Math.round(CelestinaTheme.fontCaption * window.interfaceTextScale)
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
                font.pixelSize: Math.round(CelestinaTheme.fontCaption * window.interfaceTextScale)
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
                    font.pixelSize: Math.round(CelestinaTheme.fontCaption * window.interfaceTextScale)
                    font.weight: CelestinaTheme.weightMedium
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                }

                background: Rectangle {
                    radius: CelestinaTheme.radiusSm
                    color: (sizeButton.hovered || sizePopup.opened)
                           ? CelestinaTheme.surfaceHover
                           : CelestinaTheme.controlFill
                    border.width: sizeButton.activeFocus ? 1 : 0
                    border.color: CelestinaTheme.focus

                    Behavior on color {
                        ColorAnimation { duration: CelestinaTheme.motionFast }
                    }
                }

                Popup {
                    id: sizePopup
                    // Float above the button, right-aligned to it.
                    y: -height - 10
                    x: sizeButton.width - width
                    padding: 16
                    // Modal (but undimmed) so a click anywhere outside — including
                    // a right-click that would otherwise open a context menu on
                    // top — is caught and closes it, instead of leaving both open.
                    modal: true
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
                            value: window.contentIconScale
                            maxValue: 3.0
                            onMoved: function(v) {
                                window.contentIconScale = v
                                window.persistSizing()
                            }
                        }
                        SizeRow {
                            label: "Interfaz"
                            value: window.interfaceIconScale
                            onMoved: function(v) {
                                window.interfaceIconScale = v
                                window.persistSizing()
                            }
                        }
                        SizeRow {
                            label: "Barra lateral"
                            value: window.sidebarIconScale
                            onMoved: function(v) {
                                window.sidebarIconScale = v
                                window.persistSizing()
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
                            value: window.contentTextScale
                            onMoved: function(v) {
                                window.contentTextScale = v
                                window.persistSizing()
                            }
                        }
                        SizeRow {
                            label: "Interfaz"
                            value: window.interfaceTextScale
                            onMoved: function(v) {
                                window.interfaceTextScale = v
                                window.persistSizing()
                            }
                        }
                        SizeRow {
                            label: "Barra lateral"
                            value: window.sidebarTextScale
                            onMoved: function(v) {
                                window.sidebarTextScale = v
                                window.persistSizing()
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
                                font.pixelSize: Math.round(CelestinaTheme.fontLabel * window.interfaceTextScale)
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
                                    font.pixelSize: Math.round(CelestinaTheme.fontLabel * window.interfaceTextScale)
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
                    font.pixelSize: Math.round(CelestinaTheme.fontLabel * window.interfaceTextScale)
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
                                           Math.max(190, 180 * window.interfaceTextScale)))
                height: CelestinaTheme.controlHeight
                x: topBar.width - width - 14
                anchors.verticalCenter: parent.verticalCenter
                placeholderText: "Buscar aquí y en subcarpetas"
                color: CelestinaTheme.text
                placeholderTextColor: CelestinaTheme.textMuted
                selectionColor: CelestinaTheme.accentStrong
                selectedTextColor: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: Math.round(CelestinaTheme.fontBody * window.interfaceTextScale)
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
            visible: root.tabHost !== undefined && root.tabHost.tabsModel.count >= 2

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
                model: root.tabHost ? root.tabHost.tabsModel : null
                currentIndex: root.tabHost ? root.tabHost.currentTabIndex : 0
                boundsBehavior: Flickable.StopAtBounds
                flickableDirection: Flickable.HorizontalFlick

                Connections {
                    target: root.tabHost
                    function onCurrentTabIndexChanged() {
                        tabList.positionViewAtIndex(root.tabHost.currentTabIndex,
                                                    ListView.Contain)
                    }
                }
                // Chips move relative to the backdrop when the strip scrolls.
                onContentXChanged: if (topBar.floating) topBar.glassTick()

                delegate: Item {
                    id: chip

                    required property int index
                    required property string title

                    readonly property bool activeTab: root.tabHost
                            && index === root.tabHost.currentTabIndex
                    readonly property int tabCount: root.tabHost
                            ? root.tabHost.tabsModel.count : 1

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
                        width: Math.round(CelestinaTheme.iconSm * window.interfaceIconScale)
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
                        font.pixelSize: Math.round(CelestinaTheme.fontLabel * window.interfaceTextScale)
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
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * window.interfaceTextScale)
                        }

                        MouseArea {
                            id: closeMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.tabHost.closeTab(chip.index)
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
                                root.tabHost.closeTab(chip.index)
                            else
                                root.tabHost.selectTab(chip.index)
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
                onClicked: root.tabHost.openTab(controller.currentPath, true)
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

            GlassMenuItem {
                text: entryMenu.targetDirectory ? "Abrir carpeta" : "Abrir"
                visible: !entryMenu.multi
                height: visible ? implicitHeight : 0
                icon.name: entryMenu.targetDirectory ? "folder-open" : "text-x-generic"
                icon.source: CelestinaTheme.fallbackIcon(
                                 entryMenu.targetDirectory ? "folder" : "file")
                onTriggered: controller.activateToken(entryMenu.targetToken)
            }

            GlassMenuItem {
                text: "Abrir con…"
                visible: !entryMenu.targetDirectory && !entryMenu.multi
                height: visible ? implicitHeight : 0
                icon.name: "system-run"
                icon.source: CelestinaTheme.fallbackIcon("file")
                onTriggered: controller.openWith(entryMenu.targetPath)
            }

            GlassMenuItem {
                text: "Abrir en pestaña nueva"
                visible: entryMenu.targetDirectory && !entryMenu.multi
                height: visible ? implicitHeight : 0
                icon.name: "tab-new"
                icon.source: CelestinaTheme.fallbackIcon("folder")
                onTriggered: root.requestNewTab(entryMenu.targetPath, true)
            }

            GlassMenuItem {
                text: "Añadir a marcadores"
                visible: entryMenu.targetDirectory && !entryMenu.multi
                height: visible ? implicitHeight : 0
                icon.name: "bookmark-new"
                icon.source: CelestinaTheme.fallbackIcon("folder")
                onTriggered: controller.addBookmark(entryMenu.targetPath)
            }

            GlassMenuItem {
                text: "Renombrar"
                visible: !entryMenu.multi
                height: visible ? implicitHeight : 0
                icon.name: "edit-rename"
                icon.source: CelestinaTheme.fallbackIcon("file")
                onTriggered: namePrompt.openRename(entryMenu.targetPath, entryMenu.targetName)
            }

            GlassMenuItem {
                text: entryMenu.multi
                      ? "Copiar " + entryMenu.actingCount + " elementos"
                      : "Copiar"
                icon.name: "edit-copy"
                icon.source: CelestinaTheme.fallbackIcon("file")
                onTriggered: mainPanel.copySelection(
                                 entryMenu.targetToken, entryMenu.targetPath, false)
            }

            GlassMenuItem {
                text: entryMenu.multi
                      ? "Cortar " + entryMenu.actingCount + " elementos"
                      : "Cortar"
                icon.name: "edit-cut"
                icon.source: CelestinaTheme.fallbackIcon("file")
                onTriggered: mainPanel.copySelection(
                                 entryMenu.targetToken, entryMenu.targetPath, true)
            }

            GlassMenuItem {
                text: entryMenu.multi
                      ? "Enviar " + entryMenu.actingCount + " a la papelera"
                      : "Enviar a la papelera"
                icon.name: "user-trash"
                icon.source: CelestinaTheme.fallbackIcon("file")
                onTriggered: mainPanel.trashSelection(
                                 entryMenu.targetToken, entryMenu.targetPath)
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

        // ── Name prompt (new folder / new file / rename) ─────────────────
        Rectangle {
            id: namePrompt
            anchors.fill: parent
            z: 60
            visible: false
            color: Qt.rgba(0, 0, 0, 0.45)

            property string mode: "folder"   // "folder" | "file" | "rename"
            property string targetPath: ""
            property string heading: ""

            function openCreate(kind) {
                namePrompt.mode = kind
                namePrompt.targetPath = ""
                namePrompt.heading = kind === "folder" ? "Nueva carpeta" : "Nuevo archivo"
                promptField.text = ""
                namePrompt.visible = true
                promptField.forceActiveFocus()
            }
            function openRename(path, currentName) {
                namePrompt.mode = "rename"
                namePrompt.targetPath = path
                namePrompt.heading = "Renombrar"
                promptField.text = currentName
                namePrompt.visible = true
                promptField.forceActiveFocus()
                promptField.selectAll()
            }
            function dismiss() {
                namePrompt.visible = false
                promptField.text = ""
                fileList.forceActiveFocus()
            }
            function confirm() {
                const value = promptField.text
                if (value.length === 0) {
                    namePrompt.dismiss()
                    return
                }
                if (namePrompt.mode === "folder")
                    controller.newFolder(value)
                else if (namePrompt.mode === "file")
                    controller.newFile(value)
                else
                    controller.renamePath(namePrompt.targetPath, value)
                namePrompt.dismiss()
            }

            // Click on the dimmed backdrop cancels.
            MouseArea {
                anchors.fill: parent
                onClicked: namePrompt.dismiss()
            }

            GlassCard {
                anchors.centerIn: parent
                width: Math.min(380, root.width - 48)
                height: 142
                backdropSource: mainPanel
                // Grow the whole dialog with the interface size slider, clamped
                // so it never spills past the viewport. At the default 1.0 this
                // is an exact no-op, so the glass backdrop is untouched unless
                // the user actually scales up.
                transformOrigin: Item.Center
                scale: Math.min(window.interfaceTextScale,
                                (root.width - 24) / width, (root.height - 24) / height)

                // Swallow clicks so they never reach the dismiss backdrop.
                MouseArea { anchors.fill: parent }

                Text {
                    id: promptHeading
                    x: 18
                    y: 16
                    text: namePrompt.heading
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCallout
                    font.weight: CelestinaTheme.weightDemiBold
                }

                TextField {
                    id: promptField
                    x: 18
                    y: promptHeading.y + promptHeading.height + 12
                    width: parent.width - 36
                    height: CelestinaTheme.controlHeight
                    color: CelestinaTheme.text
                    selectionColor: CelestinaTheme.accentStrong
                    selectedTextColor: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                    leftPadding: 12
                    rightPadding: 12
                    onAccepted: namePrompt.confirm()
                    Keys.onPressed: function(event) {
                        if (event.key === Qt.Key_Escape) {
                            namePrompt.dismiss()
                            event.accepted = true
                        }
                    }
                    background: Rectangle {
                        radius: CelestinaTheme.radiusSm
                        color: CelestinaTheme.inputFillFocus
                        border.width: 1
                        border.color: CelestinaTheme.focus
                    }
                }

                Row {
                    anchors.right: parent.right
                    anchors.rightMargin: 18
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 16
                    spacing: 8

                    PillButton {
                        text: "Cancelar"
                        onClicked: namePrompt.dismiss()
                    }
                    PillButton {
                        text: "Aceptar"
                        primary: true
                        onClicked: namePrompt.confirm()
                    }
                }
            }
        }

        // ── Paste conflict dialog (skip / replace / keep both) ───────────
        Rectangle {
            id: conflictDialog
            anchors.fill: parent
            z: 62
            visible: controller.conflictPending
            color: Qt.rgba(0, 0, 0, 0.45)

            // Clicking the dimmed backdrop cancels the whole paste.
            MouseArea {
                anchors.fill: parent
                onClicked: controller.cancelConflicts()
            }

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    controller.cancelConflicts()
                    event.accepted = true
                }
            }
            focus: controller.conflictPending

            GlassCard {
                anchors.centerIn: parent
                width: Math.min(420, root.width - 48)
                height: 176
                backdropSource: mainPanel
                // Scale with the interface slider, clamped to the viewport (1.0 = no-op).
                transformOrigin: Item.Center
                scale: Math.min(window.interfaceTextScale,
                                (root.width - 24) / width, (root.height - 24) / height)
                Accessible.role: Accessible.Dialog
                Accessible.name: "Conflicto al pegar"

                // Swallow clicks so they never reach the dismiss backdrop.
                MouseArea { anchors.fill: parent }

                Text {
                    id: conflictHeading
                    x: 18
                    y: 16
                    text: "Ya existe"
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCallout
                    font.weight: CelestinaTheme.weightDemiBold
                }

                Text {
                    id: conflictBody
                    x: 18
                    y: conflictHeading.y + conflictHeading.height + 10
                    width: parent.width - 36
                    wrapMode: Text.Wrap
                    text: {
                        var base = "«" + controller.conflictName
                                   + "» ya existe en esta carpeta."
                        if (controller.conflictCount > 1)
                            base += " Y " + (controller.conflictCount - 1)
                                    + " elemento(s) más. La elección se aplica a todos."
                        return base
                    }
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontLabel
                }

                Row {
                    anchors.right: parent.right
                    anchors.rightMargin: 18
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 16
                    spacing: 8

                    PillButton {
                        text: "Cancelar"
                        onClicked: controller.cancelConflicts()
                    }
                    PillButton {
                        text: "Omitir"
                        onClicked: controller.resolveConflicts("skip")
                    }
                    PillButton {
                        text: "Conservar ambos"
                        onClicked: controller.resolveConflicts("keepboth")
                    }
                    PillButton {
                        text: "Reemplazar"
                        primary: true
                        onClicked: controller.resolveConflicts("replace")
                    }
                }
            }
        }

        // ── Trash view (list + restore) ──────────────────────────────────
        Rectangle {
            id: trashView
            anchors.fill: parent
            z: 64
            // Driven by the window-scope sidebar; only the active tab shows it.
            visible: window.trashViewOpen && root.active
            color: Qt.rgba(0, 0, 0, 0.45)

            function dismiss() { window.trashViewOpen = false }

            property int focusedIndex: -1
            // Emptying is irreversible, so the button arms a confirmation rather
            // than acting on the first click.
            property bool confirmingEmpty: false
            readonly property int entryCount: controller.trashNames.length
            onVisibleChanged: {
                if (visible)
                    focusedIndex = entryCount > 0 ? 0 : -1
                confirmingEmpty = false
            }
            onFocusedIndexChanged: if (focusedIndex >= 0)
                                       trashList.positionViewAtIndex(
                                           focusedIndex, ListView.Contain)

            // Click on the dimmed backdrop closes.
            MouseArea {
                anchors.fill: parent
                onClicked: trashView.dismiss()
            }
            // Keyboard-operable: arrows move the focus, Enter restores it,
            // Escape closes.
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    trashView.dismiss()
                    event.accepted = true
                } else if (event.key === Qt.Key_Down) {
                    if (trashView.entryCount > 0)
                        trashView.focusedIndex = Math.min(
                            trashView.entryCount - 1, trashView.focusedIndex + 1)
                    event.accepted = true
                } else if (event.key === Qt.Key_Up) {
                    if (trashView.entryCount > 0)
                        trashView.focusedIndex = Math.max(
                            0, (trashView.focusedIndex < 0 ? 0 : trashView.focusedIndex) - 1)
                    event.accepted = true
                } else if ((event.key === Qt.Key_Return
                            || event.key === Qt.Key_Enter)
                           && trashView.focusedIndex >= 0) {
                    controller.restoreTrash(trashView.focusedIndex)
                    event.accepted = true
                }
            }
            focus: trashView.visible

            GlassCard {
                anchors.centerIn: parent
                width: Math.min(560, root.width - 48)
                height: Math.min(460, root.height - 64)
                backdropSource: mainPanel
                // Scale with the interface slider, clamped to the viewport (1.0 = no-op).
                transformOrigin: Item.Center
                scale: Math.min(window.interfaceTextScale,
                                (root.width - 24) / width, (root.height - 24) / height)
                Accessible.role: Accessible.Dialog
                Accessible.name: "Papelera"

                // Swallow clicks so they never reach the dismiss backdrop.
                MouseArea { anchors.fill: parent }

                Text {
                    id: trashHeading
                    x: 18
                    y: 16
                    text: "Papelera"
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCallout
                    font.weight: CelestinaTheme.weightDemiBold
                }

                Text {
                    id: trashCount
                    anchors.verticalCenter: trashHeading.verticalCenter
                    anchors.left: trashHeading.right
                    anchors.leftMargin: 8
                    text: controller.trashNames.length > 0
                          ? "· " + controller.trashNames.length : ""
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontLabel
                }

                Text {
                    anchors.centerIn: parent
                    visible: controller.trashNames.length === 0
                    text: "La papelera está vacía"
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                }

                ListView {
                    id: trashList
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    anchors.top: trashHeading.bottom
                    anchors.topMargin: 12
                    anchors.bottom: trashButtons.top
                    anchors.bottomMargin: 12
                    clip: true
                    spacing: 2
                    model: controller.trashNames

                    delegate: Item {
                        id: trashRow
                        required property int index
                        required property string modelData
                        width: ListView.view.width
                        height: 48
                        Accessible.role: Accessible.ListItem
                        Accessible.name: trashRow.modelData

                        Rectangle {
                            anchors.fill: parent
                            radius: CelestinaTheme.radiusSm
                            color: trashView.focusedIndex === trashRow.index
                                   ? CelestinaTheme.badgeAccentFill
                                   : trashRowMouse.containsMouse
                                     ? CelestinaTheme.surfaceHover : "transparent"
                        }

                        IconImage {
                            id: trashRowIcon
                            x: 8
                            anchors.verticalCenter: parent.verticalCenter
                            width: CelestinaTheme.iconSm
                            height: CelestinaTheme.iconSm
                            name: "text-x-generic"
                            source: CelestinaTheme.fallbackIcon("file")
                            color: CelestinaTheme.textMuted
                        }

                        Text {
                            id: trashRowName
                            x: trashRowIcon.x + trashRowIcon.width + 10
                            y: 6
                            width: restoreRowButton.x - x - 10
                            text: trashRow.modelData
                            color: CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontLabel
                            elide: Text.ElideMiddle
                        }

                        Text {
                            x: trashRowName.x
                            anchors.top: trashRowName.bottom
                            anchors.topMargin: 2
                            width: restoreRowButton.x - x - 10
                            text: {
                                var origin = controller.trashOrigins[trashRow.index] || ""
                                var date = controller.trashDates[trashRow.index] || ""
                                return date.length > 0 ? origin + "  ·  " + date : origin
                            }
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontCaption
                            elide: Text.ElideMiddle
                        }

                        MouseArea {
                            id: trashRowMouse
                            anchors.fill: parent
                            hoverEnabled: true
                        }

                        PillButton {
                            id: restoreRowButton
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.right: parent.right
                            anchors.rightMargin: 6
                            text: "Restaurar"
                            onClicked: controller.restoreTrash(trashRow.index)
                        }
                    }
                }

                // The "Vaciar" affordance sits apart on the left so it is not
                // mistaken for a restore; empty confirms in place before the
                // irreversible purge.
                Row {
                    anchors.left: parent.left
                    anchors.leftMargin: 18
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 16
                    spacing: 8
                    visible: controller.trashNames.length > 0

                    Text {
                        visible: trashView.confirmingEmpty
                        anchors.verticalCenter: parent.verticalCenter
                        text: "¿Vaciar? No se puede deshacer"
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                    }
                    PillButton {
                        text: trashView.confirmingEmpty ? "Vaciar definitivamente" : "Vaciar"
                        destructive: true
                        onClicked: {
                            if (trashView.confirmingEmpty) {
                                controller.emptyTrash()
                                trashView.confirmingEmpty = false
                            } else {
                                trashView.confirmingEmpty = true
                            }
                        }
                    }
                    PillButton {
                        visible: trashView.confirmingEmpty
                        text: "Cancelar"
                        onClicked: trashView.confirmingEmpty = false
                    }
                }

                Row {
                    id: trashButtons
                    anchors.right: parent.right
                    anchors.rightMargin: 18
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 16
                    spacing: 8

                    PillButton {
                        text: "Restaurar todo"
                        primary: true
                        enabled: controller.trashNames.length > 0
                        onClicked: controller.restoreAllTrash()
                    }
                    PillButton {
                        text: "Cerrar"
                        onClicked: trashView.dismiss()
                    }
                }
            }
        }

        // ── "Abrir con…" application chooser ─────────────────────────────
        Rectangle {
            id: openWithView
            anchors.fill: parent
            z: 66
            visible: controller.openWithPending
            color: Qt.rgba(0, 0, 0, 0.45)

            property int selected: -1
            readonly property int appCount: controller.openWithApps.length
            onVisibleChanged: if (visible) selected = controller.openWithDefaultIndex
            onSelectedChanged: if (selected >= 0)
                                   openWithList.positionViewAtIndex(
                                       selected, ListView.Contain)

            MouseArea {
                anchors.fill: parent
                onClicked: controller.cancelOpenWith()
            }
            // Fully keyboard-operable: arrows move the selection, Enter opens
            // it, Escape cancels.
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    controller.cancelOpenWith()
                    event.accepted = true
                } else if (event.key === Qt.Key_Down) {
                    if (openWithView.appCount > 0)
                        openWithView.selected = Math.min(
                            openWithView.appCount - 1, openWithView.selected + 1)
                    event.accepted = true
                } else if (event.key === Qt.Key_Up) {
                    if (openWithView.appCount > 0)
                        openWithView.selected = Math.max(
                            0, (openWithView.selected < 0 ? 0 : openWithView.selected) - 1)
                    event.accepted = true
                } else if ((event.key === Qt.Key_Return
                            || event.key === Qt.Key_Enter)
                           && openWithView.selected >= 0) {
                    controller.openWithApp(openWithView.selected, false)
                    event.accepted = true
                }
            }
            focus: controller.openWithPending

            GlassCard {
                anchors.centerIn: parent
                width: Math.min(480, root.width - 48)
                height: Math.min(420, root.height - 64)
                backdropSource: mainPanel
                // Scale with the interface slider, clamped to the viewport (1.0 = no-op).
                transformOrigin: Item.Center
                scale: Math.min(window.interfaceTextScale,
                                (root.width - 24) / width, (root.height - 24) / height)
                Accessible.role: Accessible.Dialog
                Accessible.name: "Abrir con"

                MouseArea { anchors.fill: parent }

                Text {
                    id: openWithHeading
                    x: 18
                    y: 16
                    width: parent.width - 36
                    text: "Abrir «" + controller.openWithTarget + "» con"
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCallout
                    font.weight: CelestinaTheme.weightDemiBold
                    elide: Text.ElideRight
                }

                Text {
                    anchors.centerIn: parent
                    visible: controller.openWithApps.length === 0
                    text: "No hay aplicaciones que declaren este tipo"
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                }

                ListView {
                    id: openWithList
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    anchors.top: openWithHeading.bottom
                    anchors.topMargin: 12
                    anchors.bottom: openWithButtons.top
                    anchors.bottomMargin: 12
                    clip: true
                    spacing: 2
                    model: controller.openWithApps

                    delegate: Item {
                        id: appRow
                        required property int index
                        required property string modelData
                        width: ListView.view.width
                        height: 38
                        Accessible.role: Accessible.ListItem
                        Accessible.name: appRow.modelData
                        Accessible.selected: openWithView.selected === appRow.index

                        Rectangle {
                            anchors.fill: parent
                            radius: CelestinaTheme.radiusSm
                            color: openWithView.selected === appRow.index
                                   ? CelestinaTheme.badgeAccentFill
                                   : appRowMouse.containsMouse
                                     ? CelestinaTheme.surfaceHover : "transparent"
                        }

                        Text {
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: defaultBadge.visible
                                   ? defaultBadge.x - x - 8 : parent.width - x - 12
                            text: appRow.modelData
                            color: openWithView.selected === appRow.index
                                   ? CelestinaTheme.accent : CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontLabel
                            elide: Text.ElideRight
                        }

                        Text {
                            id: defaultBadge
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.right: parent.right
                            anchors.rightMargin: 12
                            visible: controller.openWithDefaultIndex === appRow.index
                            text: "predeterminada"
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontMini
                        }

                        MouseArea {
                            id: appRowMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: openWithView.selected = appRow.index
                            onDoubleClicked: controller.openWithApp(appRow.index, false)
                        }
                    }
                }

                Row {
                    id: openWithButtons
                    anchors.right: parent.right
                    anchors.rightMargin: 18
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 16
                    spacing: 8

                    PillButton {
                        text: "Cancelar"
                        onClicked: controller.cancelOpenWith()
                    }
                    PillButton {
                        text: "Predeterminar y abrir"
                        enabled: openWithView.selected >= 0
                        onClicked: controller.openWithApp(openWithView.selected, true)
                    }
                    PillButton {
                        text: "Abrir"
                        primary: true
                        enabled: openWithView.selected >= 0
                        onClicked: controller.openWithApp(openWithView.selected, false)
                    }
                }
            }
        }

        // ── Properties / Get-Info panel ──────────────────────────────────
        Rectangle {
            id: propertiesView
            anchors.fill: parent
            z: 68
            visible: controller.propertiesPending
            color: Qt.rgba(0, 0, 0, 0.45)

            MouseArea {
                anchors.fill: parent
                onClicked: controller.closeProperties()
            }
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    controller.closeProperties()
                    event.accepted = true
                }
            }
            focus: controller.propertiesPending

            GlassCard {
                anchors.centerIn: parent
                width: Math.min(500, root.width - 48)
                height: Math.min(propertiesColumn.implicitHeight + propHeading.height + 90,
                                 root.height - 64)
                backdropSource: mainPanel
                // Scale with the interface slider, clamped to the viewport (1.0 = no-op).
                transformOrigin: Item.Center
                scale: Math.min(window.interfaceTextScale,
                                (root.width - 24) / width, (root.height - 24) / height)
                Accessible.role: Accessible.Dialog
                Accessible.name: "Propiedades"

                MouseArea { anchors.fill: parent }

                IconImage {
                    id: propIcon
                    x: 18
                    y: 18
                    width: CelestinaTheme.iconMd
                    height: CelestinaTheme.iconMd
                    name: controller.propIsDir ? "folder" : "text-x-generic"
                    source: CelestinaTheme.fallbackIcon(
                                controller.propIsDir ? "folder" : "file")
                    color: controller.propIsDir ? CelestinaTheme.accent
                                                : CelestinaTheme.textMuted
                }

                Text {
                    id: propHeading
                    anchors.left: propIcon.right
                    anchors.leftMargin: 12
                    anchors.right: parent.right
                    anchors.rightMargin: 18
                    y: 20
                    text: controller.propName
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCallout
                    font.weight: CelestinaTheme.weightDemiBold
                    elide: Text.ElideMiddle
                }

                Flickable {
                    id: propFlick
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.leftMargin: 18
                    anchors.rightMargin: 18
                    anchors.top: propIcon.bottom
                    anchors.topMargin: 14
                    anchors.bottom: propButtons.top
                    anchors.bottomMargin: 12
                    clip: true
                    contentHeight: propertiesColumn.implicitHeight
                    boundsBehavior: Flickable.StopAtBounds

                    Column {
                        id: propertiesColumn
                        width: propFlick.width

                        PropRow { width: parent.width; label: "Ruta"; value: controller.propPath }
                        PropRow { width: parent.width; label: "Tipo"; value: controller.propKind }
                        PropRow {
                            width: parent.width
                            label: "Enlace a"
                            value: controller.propSymlink
                        }
                        PropRow { width: parent.width; label: "MIME"; value: controller.propMime }
                        PropRow { width: parent.width; label: "Tamaño"; value: controller.propSize }
                        PropRow {
                            width: parent.width
                            label: "Permisos"
                            value: controller.propPermissions
                        }
                        PropRow {
                            width: parent.width
                            label: "Propietario"
                            value: controller.propOwner
                        }
                        PropRow {
                            width: parent.width
                            label: "Modificado"
                            value: controller.propModified
                        }
                        PropRow {
                            width: parent.width
                            label: "Accedido"
                            value: controller.propAccessed
                        }
                    }
                }

                Row {
                    id: propButtons
                    anchors.right: parent.right
                    anchors.rightMargin: 18
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 16

                    PillButton {
                        text: "Cerrar"
                        primary: true
                        onClicked: controller.closeProperties()
                    }
                }
            }
        }

        // ── Quick-look preview (spacebar) ────────────────────────────────
        // A read-only peek at the selected entry without opening an app:
        // images render full-size, text/code shows in a monospace pane, and
        // anything else (folders, video, audio, binaries) gets an info card.
        // ↑/↓ browse the folder live; Space / Esc / click-outside dismiss.
        Rectangle {
            id: quickLookView
            anchors.fill: parent
            z: 70
            visible: root.quickLookOpen
            color: Qt.rgba(0, 0, 0, 0.55)
            focus: root.quickLookOpen

            // Everything is derived from the current selection, so stepping the
            // selection (below) re-previews with no extra state to keep in sync.
            readonly property int qlIndex: controller.indexForToken(controller.selectedToken)
            readonly property string qlName: qlIndex >= 0 && qlIndex < controller.entryNames.length
                                             ? controller.entryNames[qlIndex] : ""
            readonly property string qlPath: qlIndex >= 0 ? controller.entryPath(qlIndex) : ""
            readonly property string qlKind: qlIndex >= 0 ? controller.entryKind(qlIndex) : ""
            readonly property string qlMedia: mainPanel.mediaKind(qlName)
            readonly property bool qlIsImage: qlMedia === "image"
            // Read text lazily and only when it could be text — the controller
            // returns "" for a directory, an image or a binary, which the body
            // reads as "show the info card instead".
            readonly property string qlText: (root.quickLookOpen && qlKind !== "directory"
                                              && !qlIsImage && qlPath.length > 0)
                                             ? controller.previewText(qlPath) : ""
            readonly property bool qlHasText: qlText.length > 0

            // Per-segment encode so spaces / #, ? etc. in a name survive the
            // file:// URL without mangling the path separators.
            function fileUrl(p) {
                return "file://" + p.split("/").map(encodeURIComponent).join("/")
            }

            MouseArea {
                anchors.fill: parent
                onClicked: root.quickLookOpen = false
            }
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape || event.key === Qt.Key_Space) {
                    root.quickLookOpen = false
                    event.accepted = true
                } else if (event.key === Qt.Key_Down || event.key === Qt.Key_Right) {
                    root.quickLookStep(1)
                    event.accepted = true
                } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Left) {
                    root.quickLookStep(-1)
                    event.accepted = true
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    root.quickLookOpen = false
                    controller.activateToken(controller.selectedToken)
                    event.accepted = true
                }
            }

            GlassCard {
                anchors.centerIn: parent
                width: Math.min(720, root.width - 64)
                height: Math.min(root.height - 80, 640)
                backdropSource: mainPanel
                // Scale with the interface slider, clamped to the viewport (1.0 = no-op).
                transformOrigin: Item.Center
                scale: Math.min(window.interfaceTextScale,
                                (root.width - 24) / width, (root.height - 24) / height)
                Accessible.role: Accessible.Dialog
                Accessible.name: "Vista previa"

                MouseArea { anchors.fill: parent }   // clicks on the card don't dismiss

                IconImage {
                    id: qlIcon
                    x: 18
                    y: 16
                    width: CelestinaTheme.iconSm
                    height: CelestinaTheme.iconSm
                    name: mainPanel.mediaIconName(quickLookView.qlKind, quickLookView.qlMedia, quickLookView.qlPath)
                    source: CelestinaTheme.fallbackIcon(
                                quickLookView.qlKind === "directory" ? "folder" : "file")
                    color: quickLookView.qlKind === "directory" ? CelestinaTheme.accent
                                                                : CelestinaTheme.textMuted
                }
                Text {
                    anchors.left: qlIcon.right
                    anchors.leftMargin: 10
                    anchors.right: parent.right
                    anchors.rightMargin: 18
                    y: 17
                    text: quickLookView.qlName
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCallout
                    font.weight: CelestinaTheme.weightDemiBold
                    elide: Text.ElideMiddle
                }

                Item {
                    id: qlBody
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: qlIcon.bottom
                    anchors.topMargin: 12
                    anchors.bottom: qlHint.top
                    anchors.bottomMargin: 8
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    clip: true

                    // (1) Image — the real file, capped so a huge photo can't
                    // blow up memory; not cached (previews are transient).
                    Image {
                        anchors.fill: parent
                        visible: quickLookView.qlIsImage
                        source: quickLookView.qlIsImage
                                ? quickLookView.fileUrl(quickLookView.qlPath) : ""
                        sourceSize.width: 1920
                        sourceSize.height: 1920
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        cache: false
                        smooth: true
                        mipmap: true
                    }

                    // (2) Text / code
                    ScrollView {
                        anchors.fill: parent
                        visible: !quickLookView.qlIsImage && quickLookView.qlHasText
                        clip: true
                        TextArea {
                            readOnly: true
                            text: quickLookView.qlText
                            wrapMode: TextArea.NoWrap
                            selectByMouse: true
                            background: null
                            color: CelestinaTheme.text
                            font.family: CelestinaTheme.monoFamily
                            font.pixelSize: CelestinaTheme.fontCaption
                        }
                    }

                    // (3) No renderable preview — a centred glyph + reason.
                    Column {
                        anchors.centerIn: parent
                        spacing: 12
                        visible: !quickLookView.qlIsImage && !quickLookView.qlHasText
                        IconImage {
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: 56
                            height: 56
                            name: mainPanel.mediaIconName(quickLookView.qlKind,
                                                          quickLookView.qlMedia, quickLookView.qlPath)
                            source: CelestinaTheme.fallbackIcon(
                                        quickLookView.qlKind === "directory" ? "folder" : "file")
                            color: quickLookView.qlKind === "directory"
                                   ? CelestinaTheme.accent : CelestinaTheme.textMuted
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            horizontalAlignment: Text.AlignHCenter
                            text: quickLookView.qlKind === "directory" ? "Carpeta"
                                : quickLookView.qlMedia === "video"
                                  ? "Vídeo — vista previa en Fluorita (próximamente)"
                                : quickLookView.qlMedia === "audio"
                                  ? "Audio — vista previa en Fluorita (próximamente)"
                                : "Sin vista previa"
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontBody
                        }
                    }
                }

                Text {
                    id: qlHint
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.margins: 14
                    horizontalAlignment: Text.AlignHCenter
                    text: "Espacio o Esc para cerrar   ·   ↑ ↓ para navegar"
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    opacity: 0.8
                }
            }
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
                visible: controller.searchActive || controller.searchRunning

                GlassSurface {
                    anchors.fill: parent
                    backdropSource: topBar.activeView
                    captureEnabled: searchBar.visible
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

                Text {
                    id: searchBarLabel
                    anchors.left: parent.left
                    anchors.leftMargin: 14
                    anchors.right: searchBarControls.left
                    anchors.rightMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    text: controller.searchRunning
                          ? "Buscando «" + controller.searchQuery + "»…"
                          : "«" + controller.searchQuery + "» · " + controller.searchSummary
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontLabel * window.interfaceTextScale)
                    font.weight: CelestinaTheme.weightMedium
                    elide: Text.ElideRight
                }

                Row {
                    id: searchBarControls
                    anchors.right: parent.right
                    anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 8

                    PillButton {
                        text: "Detener"
                        visible: controller.searchRunning
                        onClicked: controller.cancelSearch()
                    }
                    PillButton {
                        text: "Cerrar"
                        onClicked: controller.closeSearch()
                    }
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
                height: Math.round(CelestinaTheme.fontCaption * window.contentTextScale) + 18
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
                                font.pixelSize: Math.round(CelestinaTheme.fontCaption * window.contentTextScale)
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

            Column {
                id: placesColumn
                x: 8
                y: 42
                width: parent.width - 16
                spacing: 2

                Repeater {
                    model: [
                        { name: "Inicio", icon: "user-home", key: "HOME", fallback: "go-home" },
                        { name: "Escritorio", icon: "user-desktop", key: "DESKTOP", fallback: "folder" },
                        { name: "Documentos", icon: "folder-documents", key: "DOCUMENTS", fallback: "folder" },
                        { name: "Descargas", icon: "folder-download", key: "DOWNLOAD", fallback: "folder" },
                        { name: "Música", icon: "folder-music", key: "MUSIC", fallback: "folder" },
                        { name: "Imágenes", icon: "folder-pictures", key: "PICTURES", fallback: "folder" },
                        { name: "Vídeos", icon: "folder-videos", key: "VIDEOS", fallback: "folder" }
                    ]

                    delegate: Item {
                        id: placeRow

                        required property var modelData

                        readonly property string placePath: window.activeController
                                                            ? window.activeController.placePath(modelData.key)
                                                            : ""
                        readonly property bool available: placePath.length > 0
                        readonly property bool current: available
                                                        && placePath === (window.activeController
                                                                          ? window.activeController.currentPath : "")

                        visible: available
                        height: available ? window.sidebarRowHeight : 0
                        width: placesColumn.width

                        Rectangle {
                            anchors.fill: parent
                            anchors.leftMargin: 2
                            anchors.rightMargin: 2
                            radius: CelestinaTheme.radiusSm
                            color: placeRow.current
                                   ? CelestinaTheme.badgeAccentFill
                                   : placeMouse.containsMouse
                                     ? CelestinaTheme.surfaceHover
                                     : "transparent"
                        }

                        IconImage {
                            id: placeIcon
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                            height: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                            name: placeRow.modelData.icon
                            source: CelestinaTheme.fallbackIcon(placeRow.modelData.fallback)
                            // Native theme colours (no tint) — matches the content view.
                        }

                        Text {
                            x: placeIcon.x + placeIcon.width + 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - x - 12
                            text: placeRow.modelData.name
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
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                if (window.activeController)
                                    window.activeController.openLocation(placeRow.placePath)
                            }
                        }
                    }
                }

                // Papelera — not a folder scan; opens the Trash view overlay.
                Item {
                    id: trashPlace
                    width: placesColumn.width
                    height: window.sidebarRowHeight
                    Accessible.role: Accessible.Button
                    Accessible.name: "Papelera"
                    Accessible.onPressAction: {
                        if (window.activeController)
                            window.activeController.loadTrash()
                        window.trashViewOpen = true
                    }

                    Rectangle {
                        anchors.fill: parent
                        anchors.leftMargin: 2
                        anchors.rightMargin: 2
                        radius: CelestinaTheme.radiusSm
                        color: trashPlaceMouse.containsMouse
                               ? CelestinaTheme.surfaceHover : "transparent"
                    }

                    IconImage {
                        id: trashPlaceIcon
                        x: 12
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                        height: Math.round(CelestinaTheme.iconSm * window.sidebarIconScale)
                        name: "user-trash"
                        source: CelestinaTheme.fallbackIcon("user-trash")
                        // Native theme colours (no tint).
                    }

                    Text {
                        x: trashPlaceIcon.x + trashPlaceIcon.width + 10
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - x - 12
                        text: "Papelera"
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontBody * window.sidebarTextScale)
                        elide: Text.ElideRight
                    }

                    MouseArea {
                        id: trashPlaceMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (window.activeController)
                                window.activeController.loadTrash()
                            window.trashViewOpen = true
                        }
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
                    model: window.activeController
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
                id: bookmarksLabel
                x: 16
                y: placesColumn.y + placesColumn.height + 12
                text: "MARCADORES"
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
                // Fill down to the bottom of the (now shorter) sidebar.
                height: Math.max(0, sidebar.height - y - 14)
                clip: true
                model: window.activeController ? window.activeController.bookmarkNames : []
                spacing: 2
                boundsBehavior: Flickable.StopAtBounds

                property int editIndex: -1

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

                    width: bookmarksList.width
                    height: window.sidebarRowHeight

                    Rectangle {
                        anchors.fill: parent
                        anchors.leftMargin: 2
                        anchors.rightMargin: 2
                        radius: CelestinaTheme.radiusSm
                        color: bmRow.current
                               ? CelestinaTheme.badgeAccentFill
                               : bmMouse.containsMouse
                                 ? CelestinaTheme.surfaceHover
                                 : "transparent"
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
                        onAccepted: {
                            if (window.activeController)
                                window.activeController.renameBookmark(bmRow.index, text)
                            bookmarksList.editIndex = -1
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
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: function(mouse) {
                            if (mouse.button === Qt.RightButton) {
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

                    Document {
                        id: doc
                        anchors.fill: parent
                        active: tabHolder.visible
                        ghost: dragGhost
                        overlayParent: window.contentItem
                        tabHost: window

                        onRequestNewTab: function(path, foreground) {
                            window.openTab(path, foreground)
                        }

                        Component.onCompleted: {
                            if (tabHolder.initialPath.length > 0)
                                doc.tabController.startAt(tabHolder.initialPath)
                            else
                                doc.tabController.start()
                        }

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

    GlassContextMenu {
        id: bmMenu
        backdropSource: contentLayer

        property int targetIndex: -1

        GlassMenuItem {
            text: "Renombrar"
            onTriggered: bookmarksList.editIndex = bmMenu.targetIndex
        }

        GlassMenuItem {
            text: "Quitar de marcadores"
            onTriggered: {
                if (window.activeController)
                    window.activeController.removeBookmark(bmMenu.targetIndex)
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
        tabsModel.append({ initialPath: "", title: "…" })
        window.currentTabIndex = 0
    }
}
