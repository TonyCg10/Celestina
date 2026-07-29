import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

GridView {
    id: root

    required property var controller
    required property var entryModel
    required property var panel
    required property var hostWindow
    required property Item ghost
    required property Item overlayParent
    required property real contentTopMargin
    required property real contentBottomInset

    signal quickLookRequested
    signal revealHeadingRequested
    signal collapseHeadingRequested
    signal newTabRequested(string path, bool foreground)
    signal contextMenuRequested(string token, string name, bool isDirectory,
                                string path, real popupX, real popupY)

    readonly property int columns: Math.max(1, Math.floor(width / panel.gridCellWidth))

    FolderWheelHandler {
        view: root
        onRevealRequested: root.revealHeadingRequested()
        onCollapseRequested: root.collapseHeadingRequested()
    }

    footer: Item { width: 1; height: root.contentBottomInset }
    visible: panel.viewMode === "grid"
    model: entryModel
    clip: true
    cellWidth: Math.floor(width / columns)
    cellHeight: panel.gridCellHeight
    cacheBuffer: 480
    topMargin: contentTopMargin
    boundsBehavior: Flickable.StopAtBounds
    activeFocusOnTab: true
    keyNavigationEnabled: false
    currentIndex: -1

    Connections {
        target: root.entryModel
        function onModelReset() {
            root.currentIndex = root.controller.indexForToken(root.controller.selectedToken)
        }
    }

    function selectCell(index) {
        if (index < 0 || index >= count)
            return
        currentIndex = index
        const token = controller.entryToken(index)
        panel.selectOnly(token)
        controller.selectToken(token)
        positionViewAtIndex(index, GridView.Contain)
    }

    function pageStep() {
        const rows = Math.max(1, Math.floor(height / cellHeight))
        return rows * columns
    }

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape && root.controller.searchActive) {
            root.controller.closeSearch()
            event.accepted = true
            return
        }
        if (root.count === 0)
            return

        const index = root.currentIndex
        if (event.key === Qt.Key_Right) {
            root.selectCell(Math.min(root.count - 1, index + 1))
            event.accepted = true
        } else if (event.key === Qt.Key_Left) {
            root.selectCell(index < 0 ? root.count - 1 : Math.max(0, index - 1))
            event.accepted = true
        } else if (event.key === Qt.Key_Down) {
            root.selectCell(index < 0 ? 0 : Math.min(root.count - 1, index + root.columns))
            event.accepted = true
        } else if (event.key === Qt.Key_Up) {
            root.selectCell(index < 0 ? root.count - 1 : Math.max(0, index - root.columns))
            event.accepted = true
        } else if (event.key === Qt.Key_Home) {
            root.selectCell(0)
            event.accepted = true
        } else if (event.key === Qt.Key_End) {
            root.selectCell(root.count - 1)
            event.accepted = true
        } else if (event.key === Qt.Key_PageDown) {
            root.selectCell(Math.min(root.count - 1,
                                     (index < 0 ? 0 : index) + root.pageStep()))
            event.accepted = true
        } else if (event.key === Qt.Key_PageUp) {
            root.selectCell(Math.max(0, (index < 0 ? 0 : index) - root.pageStep()))
            event.accepted = true
        } else if (event.key === Qt.Key_Backspace) {
            if (root.controller.canGoUp && !root.controller.loading)
                root.controller.goUp()
            event.accepted = true
        } else if (index >= 0
                   && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
            root.controller.activateToken(root.controller.entryToken(index))
            event.accepted = true
        } else if (event.key === Qt.Key_Space
                   && root.controller.selectedToken.length > 0) {
            root.quickLookRequested()
            event.accepted = true
        } else if (event.modifiers === Qt.NoModifier
                   && event.text.length === 1
                   && event.text !== " " && event.text >= " ") {
            const character = event.text.toLowerCase()
            const start = index < 0 ? -1 : index
            for (let offset = 1; offset <= root.count; offset++) {
                const candidate = (start + offset) % root.count
                const name = root.controller.entryNames[candidate]
                if (name && name.toLowerCase().indexOf(character) === 0) {
                    root.selectCell(candidate)
                    break
                }
            }
            event.accepted = true
        }
    }

    delegate: FolderCellDelegate {
        panel: root.panel
        controller: root.controller
        view: root
        hostWindow: root.hostWindow
        ghost: root.ghost
        overlayParent: root.overlayParent
        onNewTabRequested: function(path, foreground) {
            root.newTabRequested(path, foreground)
        }
        onContextMenuRequested: function(token, name, isDir, path, x, y) {
            root.contextMenuRequested(token, name, isDir, path, x, y)
        }
    }

}
