import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
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
                   : pill.primary ? CelestinaTheme.canvas : CelestinaTheme.text
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
                return pill.down ? CelestinaTheme.surfaceStrong
                     : pill.hovered ? CelestinaTheme.surfaceHover
                     : CelestinaTheme.controlFill
            }
            border.width: pill.primary ? 0 : 1
            border.color: pill.activeFocus
                          ? CelestinaTheme.focus : CelestinaTheme.border

            Behavior on color {
                ColorAnimation { duration: CelestinaTheme.motionFast }
            }
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
            if (window.activeController)
                window.activeController.loadVolumes()
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
            function onRowsReady(names, tokens, kinds, subtitles, paths) {
                entryModel.setRows(names, tokens, kinds, subtitles, paths)
            }
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
                const i = topBar.activeView.currentIndex
                if (i >= 0)
                    mainPanel.trashSelection(controller.entryToken(i),
                                             controller.entryPath(i))
            }
        }

        Shortcut {
            sequences: [StandardKey.Copy]
            enabled: root.active
            onActivated: {
                const i = topBar.activeView.currentIndex
                if (i >= 0)
                    mainPanel.copySelection(controller.entryToken(i),
                                            controller.entryPath(i), false)
            }
        }

        Shortcut {
            sequences: [StandardKey.Cut]
            enabled: root.active
            onActivated: {
                const i = topBar.activeView.currentIndex
                if (i >= 0)
                    mainPanel.copySelection(controller.entryToken(i),
                                            controller.entryPath(i), true)
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

                if (button === Qt.BackButton && controller.canGoBack)
                    controller.goBack()
                else if (button === Qt.ForwardButton
                         && controller.canGoForward)
                    controller.goForward()
            }
        }

        Rectangle {
            id: mainPanel

            property string viewMode: "list"   // "list" | "grid"
            property real itemScale: 1.0        // view zoom: row/cell + icon size

            // Restore the last-used view mode and scale on open, and persist any
            // change (list⇄grid, slider) so the choice survives restarts.
            Component.onCompleted: {
                viewMode = controller.savedViewMode()
                itemScale = controller.savedScale()
            }
            function persist() {
                controller.saveViewConfig(viewMode, itemScale)
            }

            readonly property int listRowHeight: Math.round(CelestinaTheme.rowHeight * itemScale)
            readonly property int gridCellWidth: Math.round(132 * itemScale)
            readonly property int gridCellHeight: Math.round(112 * itemScale)

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

            Connections {
                target: controller
                function onCurrentPathChanged() { mainPanel.clearSelection() }
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

            Rectangle {
                id: hiddenToggle
                x: 16
                anchors.verticalCenter: bottomBar.verticalCenter
                width: hiddenLabel.width + 20
                height: 26
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
                    font.pixelSize: CelestinaTheme.fontMini
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

            BusyIndicator {
                id: busy
                width: 26
                height: 26
                x: viewToggle.x + viewToggle.width + 12
                anchors.verticalCenter: bottomBar.verticalCenter
                running: controller.loading
                visible: running
            }

            NavButton {
                id: sortDirectionButton
                x: sortButton.x + sortButton.width + 8
                anchors.verticalCenter: bottomBar.verticalCenter
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

            Button {
                id: sortButton

                readonly property var labels: [
                    "Nombre", "Tamaño", "Fecha", "Tipo"
                ]

                x: hiddenToggle.x + hiddenToggle.width + 12
                anchors.verticalCenter: bottomBar.verticalCenter
                width: 116
                height: 34
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
                    font.pixelSize: CelestinaTheme.fontCaption
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
                        ColorAnimation {
                            duration: CelestinaTheme.motionFast
                        }
                    }
                }
            }

            Button {
                id: viewToggle

                readonly property bool grid: mainPanel.viewMode === "grid"

                x: sortDirectionButton.x + sortDirectionButton.width + 10
                anchors.verticalCenter: bottomBar.verticalCenter
                width: 116
                height: 34
                text: grid ? "Lista" : "Cuadrícula"
                Accessible.name: "Cambiar vista"
                onClicked: {
                    mainPanel.viewMode = grid ? "list" : "grid"
                    mainPanel.persist()
                }

                contentItem: Text {
                    text: viewToggle.text
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    font.weight: CelestinaTheme.weightMedium
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                }

                background: Rectangle {
                    radius: CelestinaTheme.radiusSm
                    color: viewToggle.hovered
                           ? CelestinaTheme.surfaceHover
                           : CelestinaTheme.controlFill
                    border.width: viewToggle.activeFocus ? 1 : 0
                    border.color: CelestinaTheme.focus

                    Behavior on color {
                        ColorAnimation {
                            duration: CelestinaTheme.motionFast
                        }
                    }
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
                visible: mainPanel.viewMode === "list"
                model: entryModel
                clip: true
                spacing: 2
                reuseItems: true
                cacheBuffer: 420
                topMargin: 62 + (tabBar.visible ? tabBar.height + 8 : 0)
                boundsBehavior: Flickable.StopAtBounds
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
                    } else if (i >= 0 && event.key === Qt.Key_Space) {
                        controller.selectToken(controller.entryToken(i))
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

                    readonly property bool selected: mainPanel.isSelected(token)
                    // Hidden (dotfile) entries are dimmed so they read as a
                    // distinct, secondary block.
                    readonly property bool hidden: name.charAt(0) === "."

                    width: fileList.width
                    height: mainPanel.listRowHeight
                    opacity: hidden ? 0.5 : 1.0
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
                        width: Math.round(CelestinaTheme.glyphTile * mainPanel.itemScale)
                        height: Math.round(CelestinaTheme.glyphTile * mainPanel.itemScale)
                        radius: CelestinaTheme.radiusSm
                        color: row.kind === "directory"
                               ? CelestinaTheme.glyphDirectory
                               : row.kind === "symlink"
                                 ? CelestinaTheme.glyphSymlink
                                 : CelestinaTheme.glyphFile

                        IconImage {
                            anchors.centerIn: parent
                            width: Math.round(CelestinaTheme.iconMd * mainPanel.itemScale)
                            height: Math.round(CelestinaTheme.iconMd * mainPanel.itemScale)
                            name: row.kind === "directory"
                                  ? "folder"
                                  : row.kind === "symlink"
                                    ? "emblem-symbolic-link"
                                    : "text-x-generic"
                            source: CelestinaTheme.fallbackIcon(
                                        row.kind === "directory"
                                        ? "folder"
                                        : row.kind === "symlink"
                                          ? "symlink"
                                          : "file")
                            color: row.kind === "directory"
                                   ? CelestinaTheme.accent
                                   : CelestinaTheme.textMuted
                        }
                    }

                    Column {
                        id: rowText
                        x: kindGlyph.x + kindGlyph.width + 12
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - x - 24
                        spacing: 1

                        Text {
                            width: parent.width
                            text: row.name
                            color: CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * mainPanel.itemScale)
                            font.weight: CelestinaTheme.weightMedium
                            elide: Text.ElideMiddle
                        }

                        Text {
                            width: parent.width
                            text: row.subtitle
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontCaption * mainPanel.itemScale)
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
                            if (active) {
                                root.ghost.beginEntryDrag(
                                    row.path,
                                    row.name,
                                    row.isDirectory)
                            } else {
                                root.ghost.Drag.drop()
                                root.ghost.Drag.active = false
                            }
                        }
                        onCentroidChanged: {
                            if (active) {
                                root.ghost.x = centroid.scenePosition.x
                                              - root.ghost.Drag.hotSpot.x
                                root.ghost.y = centroid.scenePosition.y
                                              - root.ghost.Drag.hotSpot.y
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
                visible: mainPanel.viewMode === "grid"
                model: entryModel
                clip: true
                cellWidth: mainPanel.gridCellWidth
                cellHeight: mainPanel.gridCellHeight
                cacheBuffer: 480
                topMargin: 62 + (tabBar.visible ? tabBar.height + 8 : 0)
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
                    return Math.max(1, Math.floor(width / cellWidth))
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
                    } else if (i >= 0 && event.key === Qt.Key_Space) {
                        controller.selectToken(controller.entryToken(i))
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

                    width: fileGrid.cellWidth
                    height: fileGrid.cellHeight
                    opacity: hidden ? 0.5 : 1.0
                    Accessible.role: Accessible.ListItem
                    Accessible.name: name
                    Accessible.selected: selected

                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: 5
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
                            anchors.fill: parent
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
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: Math.round(48 * mainPanel.itemScale)
                            height: Math.round(48 * mainPanel.itemScale)
                            radius: CelestinaTheme.radiusSm
                            color: cell.kind === "directory"
                                   ? CelestinaTheme.glyphDirectory
                                   : cell.kind === "symlink"
                                     ? CelestinaTheme.glyphSymlink
                                     : CelestinaTheme.glyphFile

                            IconImage {
                                anchors.centerIn: parent
                                width: Math.round(26 * mainPanel.itemScale)
                                height: Math.round(26 * mainPanel.itemScale)
                                name: cell.kind === "directory"
                                      ? "folder"
                                      : cell.kind === "symlink"
                                        ? "emblem-symbolic-link"
                                        : "text-x-generic"
                                source: CelestinaTheme.fallbackIcon(
                                            cell.kind === "directory"
                                            ? "folder"
                                            : cell.kind === "symlink"
                                              ? "symlink"
                                              : "file")
                                color: cell.kind === "directory"
                                       ? CelestinaTheme.accent
                                       : CelestinaTheme.textMuted
                            }
                        }

                        Text {
                            width: fileGrid.cellWidth - 22
                            horizontalAlignment: Text.AlignHCenter
                            text: cell.name
                            color: CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontCaption * mainPanel.itemScale)
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
                            if (active) {
                                root.ghost.beginEntryDrag(
                                    cell.path,
                                    cell.name,
                                    cell.isDirectory)
                            } else {
                                root.ghost.Drag.drop()
                                root.ghost.Drag.active = false
                            }
                        }
                        onCentroidChanged: {
                            if (active) {
                                root.ghost.x = centroid.scenePosition.x
                                              - root.ghost.Drag.hotSpot.x
                                root.ghost.y = centroid.scenePosition.y
                                              - root.ghost.Drag.hotSpot.y
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

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: controller.query.length > 0 ? "Sin coincidencias" : "Carpeta vacía"
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontTitle
                    font.weight: CelestinaTheme.weightMedium
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: controller.query.length > 0
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
                    font.pixelSize: CelestinaTheme.fontCaption
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
                x: busy.x + busy.width + 14
                anchors.verticalCenter: bottomBar.verticalCenter
                width: Math.max(0, zoomSlider.x - x - 12)
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
                font.pixelSize: CelestinaTheme.fontCaption
                elide: Text.ElideRight
            }

            Slider {
                id: zoomSlider

                from: 0.8
                to: 1.9
                value: mainPanel.itemScale
                stepSize: 0.1
                width: 116
                height: 22
                anchors.right: parent.right
                anchors.rightMargin: 16
                anchors.verticalCenter: bottomBar.verticalCenter
                Accessible.name: "Tamaño de los elementos"
                onMoved: {
                    mainPanel.itemScale = value
                    mainPanel.persist()
                }

                background: Rectangle {
                    x: zoomSlider.leftPadding
                    y: zoomSlider.topPadding + zoomSlider.availableHeight / 2 - height / 2
                    width: zoomSlider.availableWidth
                    height: 4
                    radius: 2
                    color: CelestinaTheme.controlFill

                    Rectangle {
                        width: zoomSlider.visualPosition * parent.width
                        height: parent.height
                        radius: 2
                        color: CelestinaTheme.accent
                    }
                }

                handle: Rectangle {
                    x: zoomSlider.leftPadding
                       + zoomSlider.visualPosition * (zoomSlider.availableWidth - width)
                    y: zoomSlider.topPadding + zoomSlider.availableHeight / 2 - height / 2
                    width: 15
                    height: 15
                    radius: 7.5
                    color: zoomSlider.pressed ? CelestinaTheme.accent : CelestinaTheme.text
                    border.width: 1
                    border.color: CelestinaTheme.borderStrong
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
                                font.pixelSize: CelestinaTheme.fontLabel
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
                                    font.pixelSize: CelestinaTheme.fontLabel
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
                    font.pixelSize: CelestinaTheme.fontLabel
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
                width: Math.min(260, Math.max(170, topBar.width * 0.24))
                height: CelestinaTheme.controlHeight
                x: topBar.width - width - 14
                anchors.verticalCenter: parent.verticalCenter
                placeholderText: "Filtrar · ⏎ busca en subcarpetas"
                color: CelestinaTheme.text
                placeholderTextColor: CelestinaTheme.textMuted
                selectionColor: CelestinaTheme.accentStrong
                selectedTextColor: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                leftPadding: 13
                rightPadding: 13
                onTextEdited: searchDebounce.restart()
                // Enter runs a recursive filename search of the current folder.
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
                interval: 120
                repeat: false
                onTriggered: controller.applyQuery(searchField.text)
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

                    width: 172
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
                        width: CelestinaTheme.iconSm
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
                        font.pixelSize: CelestinaTheme.fontLabel
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
                            font.pixelSize: CelestinaTheme.fontBody
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
            readonly property int entryCount: controller.trashNames.length
            onVisibleChanged: if (visible) focusedIndex = entryCount > 0 ? 0 : -1
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

        // ── Recursive filename search results ────────────────────────────
        Rectangle {
            id: searchView
            anchors.fill: parent
            z: 70
            visible: controller.searchActive
            color: Qt.rgba(0, 0, 0, 0.45)

            MouseArea {
                anchors.fill: parent
                onClicked: controller.closeSearch()
            }
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    controller.closeSearch()
                    event.accepted = true
                }
            }
            focus: controller.searchActive

            GlassCard {
                anchors.centerIn: parent
                width: Math.min(600, root.width - 48)
                height: Math.min(520, root.height - 64)
                backdropSource: mainPanel
                Accessible.role: Accessible.Dialog
                Accessible.name: "Resultados de búsqueda"

                MouseArea { anchors.fill: parent }

                Text {
                    id: searchHeading
                    x: 18
                    y: 16
                    width: parent.width - 36
                    text: "Buscar «" + controller.searchQuery + "»"
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCallout
                    font.weight: CelestinaTheme.weightDemiBold
                    elide: Text.ElideRight
                }

                Text {
                    id: searchSummary
                    anchors.left: searchHeading.left
                    anchors.top: searchHeading.bottom
                    anchors.topMargin: 3
                    text: controller.searchSummary
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                }

                Text {
                    anchors.centerIn: parent
                    visible: !controller.searchRunning
                             && controller.searchNames.length === 0
                    text: "Sin coincidencias"
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                }

                ListView {
                    id: searchList
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    anchors.top: searchSummary.bottom
                    anchors.topMargin: 12
                    anchors.bottom: searchButtons.top
                    anchors.bottomMargin: 12
                    clip: true
                    spacing: 2
                    model: controller.searchNames

                    delegate: Item {
                        id: hitRow
                        required property int index
                        required property string modelData
                        readonly property bool isDir:
                            (controller.searchKinds[index] || "") === "directory"
                        width: ListView.view.width
                        height: 46
                        Accessible.role: Accessible.ListItem
                        Accessible.name: hitRow.modelData

                        Rectangle {
                            anchors.fill: parent
                            radius: CelestinaTheme.radiusSm
                            color: hitMouse.containsMouse
                                   ? CelestinaTheme.surfaceHover : "transparent"
                        }

                        IconImage {
                            id: hitIcon
                            x: 8
                            anchors.verticalCenter: parent.verticalCenter
                            width: CelestinaTheme.iconSm
                            height: CelestinaTheme.iconSm
                            name: hitRow.isDir ? "folder" : "text-x-generic"
                            source: CelestinaTheme.fallbackIcon(
                                        hitRow.isDir ? "folder" : "file")
                            color: hitRow.isDir ? CelestinaTheme.accent
                                                : CelestinaTheme.textMuted
                        }

                        Text {
                            id: hitName
                            x: hitIcon.x + hitIcon.width + 10
                            y: 5
                            width: parent.width - x - 12
                            text: hitRow.modelData
                            color: CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontLabel
                            elide: Text.ElideMiddle
                        }

                        Text {
                            x: hitName.x
                            anchors.top: hitName.bottom
                            anchors.topMargin: 1
                            width: parent.width - x - 12
                            text: controller.searchPaths[hitRow.index] || ""
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontMini
                            elide: Text.ElideMiddle
                        }

                        MouseArea {
                            id: hitMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: controller.openSearchHit(hitRow.index)
                        }
                    }
                }

                Row {
                    id: searchButtons
                    anchors.right: parent.right
                    anchors.rightMargin: 18
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 16
                    spacing: 8

                    PillButton {
                        text: "Detener"
                        visible: controller.searchRunning
                        onClicked: controller.cancelSearch()
                    }
                    PillButton {
                        text: "Cerrar"
                        primary: true
                        onClicked: controller.closeSearch()
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

        // Floating ghost shown while dragging a folder toward the sidebar.
        Item {
            id: dragGhost
            z: 100
            width: 150
            height: 34
            visible: Drag.active
            property string path: ""
            property string label: ""
            property bool isDir: false
            Drag.hotSpot.x: 16
            Drag.hotSpot.y: 17
            // Automatic so the drag also reaches other applications as a
            // text/uri-list; internal DropAreas still match on our keys.
            Drag.dragType: Drag.Automatic
            Drag.supportedActions: Qt.CopyAction | Qt.MoveAction

            // Start dragging an entry: only folders carry the bookmark key (so a
            // file can't be dropped on the sidebar), every entry carries the
            // move key for folder-to-folder drops, and a file:// URI so other
            // apps can accept the drop.
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
                dragGhost.Drag.active = true
            }

            Rectangle {
                anchors.fill: parent
                radius: CelestinaTheme.radiusSm
                color: CelestinaTheme.surfaceStrong
                border.width: 1
                border.color: CelestinaTheme.accent
                opacity: 0.96

                Row {
                    x: 10
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 8

                    IconImage {
                        anchors.verticalCenter: parent.verticalCenter
                        width: CelestinaTheme.iconSm
                        height: CelestinaTheme.iconSm
                        name: dragGhost.isDir ? "folder" : "text-x-generic"
                        source: CelestinaTheme.fallbackIcon(
                                    dragGhost.isDir ? "folder" : "file")
                        color: CelestinaTheme.accent
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 104
                        text: dragGhost.label
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontLabel
                        elide: Text.ElideRight
                    }
                }
            }
        }

        Rectangle {
            id: sidebar
            x: 20
            y: 18
            width: 184
            // Leave room below for the separate item-info box (64 + gap).
            height: parent.height - y - 18 - 78
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
                        height: available ? 34 : 0
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
                            width: CelestinaTheme.iconSm
                            height: CelestinaTheme.iconSm
                            name: placeRow.modelData.icon
                            source: CelestinaTheme.fallbackIcon(placeRow.modelData.fallback)
                            color: placeRow.current ? CelestinaTheme.accent
                                                    : CelestinaTheme.textMuted
                        }

                        Text {
                            x: placeIcon.x + placeIcon.width + 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - x - 12
                            text: placeRow.modelData.name
                            color: placeRow.current ? CelestinaTheme.accent
                                                    : CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontLabel
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
                    height: 34
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
                        width: CelestinaTheme.iconSm
                        height: CelestinaTheme.iconSm
                        name: "user-trash"
                        source: CelestinaTheme.fallbackIcon("user-trash")
                        color: CelestinaTheme.textMuted
                    }

                    Text {
                        x: trashPlaceIcon.x + trashPlaceIcon.width + 10
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - x - 12
                        text: "Papelera"
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontLabel
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
                        font.pixelSize: CelestinaTheme.fontMini
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
                        font.pixelSize: CelestinaTheme.fontMini

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
                        height: 34
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
                            width: CelestinaTheme.iconSm
                            height: CelestinaTheme.iconSm
                            name: "drive-removable-media"
                            source: CelestinaTheme.fallbackIcon("folder")
                            color: volumeRow.current ? CelestinaTheme.accent
                                                     : CelestinaTheme.textMuted
                        }

                        Text {
                            x: volumeIcon.x + volumeIcon.width + 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: ejectButton.x - x - 6
                            text: volumeRow.modelData
                            color: volumeRow.current ? CelestinaTheme.accent
                                                     : CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontLabel
                            elide: Text.ElideRight
                        }

                        // Eject (unmount) when mounted; hidden otherwise.
                        IconImage {
                            id: ejectButton
                            z: 3   // above the full-row open handler below
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.right: parent.right
                            anchors.rightMargin: 10
                            width: CelestinaTheme.iconSm
                            height: CelestinaTheme.iconSm
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
                font.pixelSize: CelestinaTheme.fontMini
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
                    height: 34

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
                        width: CelestinaTheme.iconSm
                        height: CelestinaTheme.iconSm
                        name: "folder"
                        source: CelestinaTheme.fallbackIcon("folder")
                        color: bmRow.current ? CelestinaTheme.accent
                                             : CelestinaTheme.textMuted
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
                        font.pixelSize: CelestinaTheme.fontLabel
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
            height: 64
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
                x: 16
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 32
                spacing: 2

                Text {
                    text: sidebarInfo.header
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontMini
                    font.letterSpacing: 1.4
                    font.weight: CelestinaTheme.weightDemiBold
                }

                Text {
                    width: parent.width
                    text: sidebarInfo.primary
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                    font.weight: CelestinaTheme.weightMedium
                    elide: Text.ElideMiddle
                }

                Text {
                    width: parent.width
                    visible: sidebarInfo.secondary.length > 0
                    text: sidebarInfo.secondary
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
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
