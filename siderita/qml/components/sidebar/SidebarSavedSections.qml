import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

Item {
    id: root

    required property var hostWindow
    required property Item overlayParent

    signal favoriteMenuRequested(string path, real popupX, real popupY)
    signal bookmarkMenuRequested(int index, string path, real popupX, real popupY)

    // Remembered between runs, like the other two sections: the controller
    // holds the list and these read it.
    readonly property bool favoritesCollapsed: root.folded("favorites")
    readonly property bool bookmarksCollapsed: root.folded("bookmarks")

    function folded(section) {
        const controller = root.hostWindow.activeController
        return controller ? controller.collapsedSections.indexOf(section) >= 0 : false
    }
    function fold(section, collapsed) {
        const controller = root.hostWindow.activeController
        if (controller)
            controller.setSectionCollapsed(section, collapsed)
    }

    readonly property int bookmarkCount: bookmarksList.count
    readonly property Item favoritesHeaderItem: favoritesHeader
    readonly property Item bookmarksHeaderItem: bookmarksHeader
    readonly property real favoritesHeaderY: favoritesHeader.y
    readonly property real bookmarksHeaderY: bookmarksHeader.y
    readonly property var favoriteRows: {
        const rows = []
        const controller = hostWindow.activeController
        const entries = controller ? controller.favoriteEntries : []
        for (let index = 0; index < entries.length; index++) {
            const cut = entries[index].indexOf("\t")
            if (cut <= 0)
                continue
            // The first field is a path key, so the name a person reads comes
            // from the controller rather than from cutting the key up.
            const path = entries[index].substring(0, cut)
            rows.push({
                path: path,
                kind: entries[index].substring(cut + 1),
                name: controller.displayLocationName(path)
            })
        }
        return rows
    }

    implicitHeight: bookmarksList.y + bookmarksList.height + 14
    height: implicitHeight

    function editBookmark(index) {
        bookmarksList.editIndex = index
    }

    SidebarSectionHeader {
        id: favoritesHeader
        x: CelestinaTheme.spaceSm
        y: 0
        width: parent.width - CelestinaTheme.spaceSm * 2
        visible: root.favoriteRows.length > 0
        height: visible ? implicitHeight : 0
        title: "FAVORITOS"
        textScale: root.hostWindow.sidebarTextScale
        iconScale: root.hostWindow.sidebarIconScale
        collapsed: root.favoritesCollapsed
        onActivated: root.fold("favorites", !root.favoritesCollapsed)
    }

    ListView {
        id: favoritesList
        x: 8
        y: favoritesHeader.y + favoritesHeader.height
        width: parent.width - 16
        height: root.favoritesCollapsed
                ? 0 : count * (root.hostWindow.sidebarRowHeight + spacing)
        interactive: false
        visible: count > 0 && !root.favoritesCollapsed
        clip: true
        model: root.favoriteRows
        spacing: 2
        boundsBehavior: Flickable.StopAtBounds

        delegate: SidebarFavoriteRow {
            required property var modelData
            width: favoritesList.width
            hostWindow: root.hostWindow
            overlayParent: root.overlayParent
            entry: modelData
            onContextMenuRequested: function(path, popupX, popupY) {
                root.favoriteMenuRequested(path, popupX, popupY)
            }
        }
    }

    SidebarSectionHeader {
        id: bookmarksHeader
        x: CelestinaTheme.spaceSm
        width: parent.width - CelestinaTheme.spaceSm * 2
        y: favoritesHeader.visible
           ? favoritesList.y + favoritesList.height + 12
           : favoritesHeader.y
        title: "MARCADORES"
        textScale: root.hostWindow.sidebarTextScale
        iconScale: root.hostWindow.sidebarIconScale
        collapsed: root.bookmarksCollapsed
        onActivated: root.fold("bookmarks", !root.bookmarksCollapsed)
    }

    ListView {
        id: bookmarksList
        x: 8
        y: bookmarksHeader.y + bookmarksHeader.height
        width: parent.width - 16
        height: root.bookmarksCollapsed
                ? 0 : count * (root.hostWindow.sidebarRowHeight + spacing)
        visible: !root.bookmarksCollapsed
        clip: true
        interactive: false
        model: root.hostWindow.activeController
               ? root.hostWindow.activeController.bookmarkNames : []
        spacing: 2
        boundsBehavior: Flickable.StopAtBounds

        property int editIndex: -1
        property int dragIndex: -1
        property int dropIndex: -1
        readonly property int rowPitch: root.hostWindow.sidebarRowHeight + spacing

        function finishMove(from, to) {
            dragIndex = -1
            dropIndex = -1
            if (to >= 0 && to !== from && root.hostWindow.activeController)
                root.hostWindow.activeController.moveBookmark(from, to)
        }

        delegate: SidebarBookmarkRow {
            required property int index
            required property string modelData

            readonly property string rowPath:
                (root.hostWindow.activeController
                 && index >= 0
                 && index < root.hostWindow.activeController.bookmarkPaths.length)
                ? root.hostWindow.activeController.bookmarkPaths[index] : ""

            width: bookmarksList.width
            hostWindow: root.hostWindow
            overlayParent: root.overlayParent
            rowIndex: index
            bookmarkName: modelData
            bookmarkPath: rowPath
            editing: bookmarksList.editIndex === index
            listDragIndex: bookmarksList.dragIndex
            listDropIndex: bookmarksList.dropIndex
            rowPitch: bookmarksList.rowPitch
            rowCount: bookmarksList.count

            onEditRequested: function(row) { bookmarksList.editIndex = row }
            onEditCancelled: bookmarksList.editIndex = -1
            onRenameRequested: function(row, value) {
                bookmarksList.editIndex = -1
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.renameBookmark(row, value)
            }
            onDragMoved: function(target) {
                bookmarksList.dragIndex = index
                bookmarksList.dropIndex = target
            }
            onDragFinished: function(from, to) { bookmarksList.finishMove(from, to) }
            onDragCancelled: {
                bookmarksList.dragIndex = -1
                bookmarksList.dropIndex = -1
            }
            onContextMenuRequested: function(row, path, popupX, popupY) {
                root.bookmarkMenuRequested(row, path, popupX, popupY)
            }
        }
    }
}
