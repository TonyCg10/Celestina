import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import org.celestina.siderita 1.0

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
            sequence: StandardKey.Copy
            enabled: root.active
            onActivated: {
                const i = topBar.activeView.currentIndex
                if (i >= 0)
                    mainPanel.copySelection(controller.entryToken(i),
                                            controller.entryPath(i), false)
            }
        }

        Shortcut {
            sequence: StandardKey.Cut
            enabled: root.active
            onActivated: {
                const i = topBar.activeView.currentIndex
                if (i >= 0)
                    mainPanel.copySelection(controller.entryToken(i),
                                            controller.entryPath(i), true)
            }
        }

        Shortcut {
            sequence: StandardKey.Paste
            enabled: root.active && controller.canPaste && !controller.opRunning
            onActivated: controller.paste()
        }

        Shortcut {
            sequence: StandardKey.Undo
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

            readonly property int listRowHeight: Math.round(CelestinaTheme.rowHeight * itemScale)
            readonly property int gridCellWidth: Math.round(132 * itemScale)
            readonly property int gridCellHeight: Math.round(112 * itemScale)

            // ── Multi-selection (token-keyed, so it survives sort/filter) ────
            property var selectedTokens: ({})
            property int selectionCount: 0
            property string anchorToken: ""

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
                onClicked: mainPanel.viewMode = grid ? "list" : "grid"

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

            ListView {
                id: fileList
                x: 8
                y: 14
                width: parent.width - 16
                height: parent.height - 68
                visible: mainPanel.viewMode === "list"
                model: controller.entryNames
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
                    target: controller

                    function onViewRevisionChanged() {
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

                    required property int index
                    required property string modelData

                    readonly property int revision: controller.viewRevision
                    readonly property string token: revision >= 0
                                                    ? controller.entryToken(index)
                                                    : ""
                    readonly property string kind: revision >= 0
                                                   ? controller.entryKind(index)
                                                   : ""
                    readonly property string subtitle: revision >= 0
                                                       ? controller.entrySubtitle(index)
                                                       : ""
                    readonly property bool selected: mainPanel.isSelected(token)

                    width: fileList.width
                    height: mainPanel.listRowHeight
                    Accessible.role: Accessible.ListItem
                    Accessible.name: modelData
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
                            text: row.modelData
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
                                if (controller.entryIsDirectory(row.index))
                                    root.requestNewTab(
                                        controller.entryPath(row.index), false)
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
                                entryMenu.targetName = row.modelData
                                entryMenu.targetDirectory =
                                        controller.entryIsDirectory(row.index)
                                entryMenu.targetPath = controller.entryPath(row.index)
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
                        enabled: row.revision >= 0
                                 && controller.entryIsDirectory(row.index)
                        onActiveChanged: {
                            if (active) {
                                root.ghost.path = controller.entryPath(row.index)
                                root.ghost.label = row.modelData
                                root.ghost.Drag.active = true
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
                model: controller.entryNames
                clip: true
                cellWidth: mainPanel.gridCellWidth
                cellHeight: mainPanel.gridCellHeight
                cacheBuffer: 480
                topMargin: 62 + (tabBar.visible ? tabBar.height + 8 : 0)
                boundsBehavior: Flickable.StopAtBounds
                currentIndex: -1

                delegate: Item {
                    id: cell

                    required property int index
                    required property string modelData

                    readonly property int revision: controller.viewRevision
                    readonly property string token: revision >= 0
                                                    ? controller.entryToken(index)
                                                    : ""
                    readonly property string kind: revision >= 0
                                                   ? controller.entryKind(index)
                                                   : ""
                    readonly property bool selected: mainPanel.isSelected(token)

                    width: fileGrid.cellWidth
                    height: fileGrid.cellHeight
                    Accessible.role: Accessible.ListItem
                    Accessible.name: modelData
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
                            text: cell.modelData
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
                                if (controller.entryIsDirectory(cell.index))
                                    root.requestNewTab(
                                        controller.entryPath(cell.index), false)
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
                                entryMenu.targetName = cell.modelData
                                entryMenu.targetDirectory =
                                        controller.entryIsDirectory(cell.index)
                                entryMenu.targetPath = controller.entryPath(cell.index)
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
                        enabled: cell.revision >= 0
                                 && controller.entryIsDirectory(cell.index)
                        onActiveChanged: {
                            if (active) {
                                root.ghost.path = controller.entryPath(cell.index)
                                root.ghost.label = cell.modelData
                                root.ghost.Drag.active = true
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
                text: mainPanel.selectionCount > 1
                      ? mainPanel.selectionCount + " seleccionados"
                      : controller.statusText
                color: CelestinaTheme.textMuted
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
                onMoved: mainPanel.itemScale = value

                ToolTip.visible: hovered || pressed
                ToolTip.text: Math.round(value * 100) + " %"

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
                    anchors.fill: parent
                    visible: !pathPill.editing
                    cursorShape: Qt.IBeamCursor
                    Accessible.name: "Editar ubicación"
                    onClicked: pathPill.beginEditing()
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
                placeholderText: "Buscar en esta carpeta"
                color: CelestinaTheme.text
                placeholderTextColor: CelestinaTheme.textMuted
                selectionColor: CelestinaTheme.accentStrong
                selectedTextColor: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                leftPadding: 13
                rightPadding: 13
                onTextEdited: searchDebounce.restart()

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

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(380, root.width - 48)
                height: 142
                radius: CelestinaTheme.radiusMd
                color: CelestinaTheme.canvasRaised
                border.width: 1
                border.color: CelestinaTheme.borderStrong

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

                    Button {
                        text: "Cancelar"
                        onClicked: namePrompt.dismiss()
                    }
                    Button {
                        text: "Aceptar"
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

            Rectangle {
                anchors.centerIn: parent
                width: Math.min(420, root.width - 48)
                height: 176
                radius: CelestinaTheme.radiusMd
                color: CelestinaTheme.canvasRaised
                border.width: 1
                border.color: CelestinaTheme.borderStrong

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

                    Button {
                        text: "Cancelar"
                        onClicked: controller.cancelConflicts()
                    }
                    Button {
                        text: "Omitir"
                        onClicked: controller.resolveConflicts("skip")
                    }
                    Button {
                        text: "Conservar ambos"
                        onClicked: controller.resolveConflicts("keepboth")
                    }
                    Button {
                        text: "Reemplazar"
                        onClicked: controller.resolveConflicts("replace")
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
            Drag.keys: ["siderita-bookmark"]
            Drag.hotSpot.x: 16
            Drag.hotSpot.y: 17

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
                        name: "folder"
                        source: CelestinaTheme.fallbackIcon("folder")
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
            height: parent.height - y - 20
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
                height: Math.max(0, readOnlyBadge.y - y - 12)
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

            Rectangle {
                id: readOnlyBadge
                x: 14
                y: sidebarInfo.y - height - 12
                width: parent.width - 28
                height: 32
                radius: CelestinaTheme.radiusSm
                color: CelestinaTheme.badgeFill
                border.width: 1
                border.color: CelestinaTheme.border

                Text {
                    anchors.centerIn: parent
                    text: "SOLO LECTURA · I1"
                    color: CelestinaTheme.accent
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontMini
                    font.weight: CelestinaTheme.weightDemiBold
                    font.letterSpacing: 0.8
                }
            }

            // Bottom info box: folder count now, room for more later.
            Rectangle {
                id: sidebarInfo
                x: 14
                width: parent.width - 28
                y: parent.height - height - 14
                height: 62
                radius: CelestinaTheme.radiusSm
                color: CelestinaTheme.badgeFill
                border.width: 1
                border.color: CelestinaTheme.border

                Column {
                    x: 14
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 28
                    spacing: 4

                    Text {
                        text: "CARPETA"
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontMini
                        font.letterSpacing: 1.4
                        font.weight: CelestinaTheme.weightDemiBold
                    }

                    Text {
                        readonly property int count: window.activeController
                                                     ? window.activeController.entryNames.length : 0
                        text: count + (count === 1 ? " elemento" : " elementos")
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontBody
                        font.weight: CelestinaTheme.weightMedium
                    }
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

    Component.onCompleted: {
        tabsModel.append({ initialPath: "", title: "…" })
        window.currentTabIndex = 0
    }
}
