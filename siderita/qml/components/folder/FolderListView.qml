import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

ListView {
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

    readonly property bool detailsMode: panel.viewMode === "details"
                                        && !controller.searchActive
    readonly property int colSizeW: Math.round(92 * hostWindow.contentTextScale)
    readonly property int colDateW: Math.round(150 * hostWindow.contentTextScale)
    readonly property int colTypeW: Math.round(96 * hostWindow.contentTextScale)
    readonly property int detailsNameX: 14
            + Math.round(CelestinaTheme.glyphTile * hostWindow.contentIconScale) + 12

    FolderWheelHandler {
        view: root
        onRevealRequested: root.revealHeadingRequested()
        onCollapseRequested: root.collapseHeadingRequested()
    }

    footer: Item { width: 1; height: root.contentBottomInset }
    visible: panel.viewMode !== "grid"
    model: entryModel
    clip: true
    spacing: 0
    reuseItems: true
    cacheBuffer: 420
    topMargin: contentTopMargin
    boundsBehavior: Flickable.StopAtBounds
    activeFocusOnTab: true
    keyNavigationEnabled: false
    currentIndex: -1

    section.property: "section"
    section.criteria: ViewSection.FullString
    section.delegate: Item {
        id: sectionHeader
        required property string section
        width: root.width
        height: sectionHeader.section.length > 0
                ? Math.round(CelestinaTheme.fontMini * root.hostWindow.contentTextScale) + 22
                : 0
        visible: sectionHeader.section.length > 0

        CelestinaSectionLabel {
            x: 14
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 6
            text: sectionHeader.section.toUpperCase()
            textScale: root.hostWindow.contentTextScale
        }
    }

    Connections {
        target: root.entryModel
        function onModelReset() {
            root.currentIndex = root.controller.indexForToken(root.controller.selectedToken)
        }
    }

    function selectRow(index) {
        if (index < 0 || index >= count)
            return
        currentIndex = index
        const token = controller.entryToken(index)
        panel.selectOnly(token)
        controller.selectToken(token)
        positionViewAtIndex(index, ListView.Contain)
    }

    function pageStep() {
        return Math.max(1, Math.floor(height / (panel.listRowHeight + spacing)))
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
        if (event.key === Qt.Key_Down) {
            root.selectRow(Math.min(root.count - 1, index + 1))
            event.accepted = true
        } else if (event.key === Qt.Key_Up) {
            root.selectRow(index < 0 ? root.count - 1 : Math.max(0, index - 1))
            event.accepted = true
        } else if (event.key === Qt.Key_Home) {
            root.selectRow(0)
            event.accepted = true
        } else if (event.key === Qt.Key_End) {
            root.selectRow(root.count - 1)
            event.accepted = true
        } else if (event.key === Qt.Key_PageDown) {
            root.selectRow(Math.min(root.count - 1,
                                    (index < 0 ? 0 : index) + root.pageStep()))
            event.accepted = true
        } else if (event.key === Qt.Key_PageUp) {
            root.selectRow(Math.max(0, (index < 0 ? 0 : index) - root.pageStep()))
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
                    root.selectRow(candidate)
                    break
                }
            }
            event.accepted = true
        }
    }

    delegate: FolderRowDelegate {
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
