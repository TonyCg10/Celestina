import QtQuick
import org.celestina.siderita 1.0

Item {
    id: root

    required property var hostWindow
    required property Item overlayParent
    required property Item backdropSource
    property int bookmarkCount: 0

    signal editBookmarkRequested(int index)

    function openBookmark(index, point) {
        bookmarkMenu.targetIndex = index
        bookmarkMenu.popup(overlayParent, point)
    }

    function openPlace(key, name, path, point) {
        placeMenu.targetKey = key
        placeMenu.targetName = name
        placeMenu.targetPath = path
        placeMenu.popup(overlayParent, point)
    }

    function openFavorite(path, point) {
        favoriteMenu.targetPath = path
        favoriteMenu.popup(overlayParent, point)
    }

    function openDevice(name, point) {
        deviceMenu.deviceName = name
        deviceMenu.popup(overlayParent, point)
    }

    GlassContextMenu {
        id: bookmarkMenu
        backdropSource: root.backdropSource
        property int targetIndex: -1

        GlassMenuItem {
            text: "Renombrar"
            onTriggered: root.editBookmarkRequested(bookmarkMenu.targetIndex)
        }

        GlassMenuItem {
            text: "Subir"
            enabled: bookmarkMenu.targetIndex > 0
            icon.name: "go-up"
            icon.source: CelestinaTheme.fallbackIcon("go-up")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.moveBookmark(
                                bookmarkMenu.targetIndex, bookmarkMenu.targetIndex - 1)
            }
        }

        GlassMenuItem {
            text: "Bajar"
            enabled: bookmarkMenu.targetIndex >= 0
                     && bookmarkMenu.targetIndex < root.bookmarkCount - 1
            icon.name: "go-down"
            icon.source: CelestinaTheme.fallbackIcon("go-up")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.moveBookmark(
                                bookmarkMenu.targetIndex, bookmarkMenu.targetIndex + 1)
            }
        }

        GlassMenuItem {
            text: "Quitar de marcadores"
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.removeBookmark(bookmarkMenu.targetIndex)
            }
        }
    }

    GlassContextMenu {
        id: placeMenu
        backdropSource: root.backdropSource
        property string targetKey: ""
        property string targetName: ""
        property string targetPath: ""

        GlassMenuItem {
            text: "Abrir en pestaña nueva"
            visible: placeMenu.targetPath.length > 0
            height: visible ? implicitHeight : 0
            icon.name: "tab-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: root.hostWindow.openTab(placeMenu.targetPath, true)
        }

        GlassMenuItem {
            text: "Ocultar «" + placeMenu.targetName + "»"
            icon.name: "list-remove"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.hidePlace(placeMenu.targetKey)
            }
        }

        GlassMenuItem {
            text: "Mostrar lugares ocultos"
            visible: root.hostWindow.activeController
                     && root.hostWindow.activeController.hiddenPlaceCount > 0
            height: visible ? implicitHeight : 0
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.unhideAllPlaces()
            }
        }
    }

    GlassContextMenu {
        id: favoriteMenu
        backdropSource: root.backdropSource
        property string targetPath: ""

        GlassMenuItem {
            text: "Abrir en pestaña nueva"
            icon.name: "tab-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: root.hostWindow.openTab(favoriteMenu.targetPath, true)
        }

        GlassMenuItem {
            text: "Mostrar en su carpeta"
            icon.name: "folder-open"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.revealPath(favoriteMenu.targetPath)
            }
        }

        GlassMenuItem {
            text: "Quitar de favoritos"
            icon.source: CelestinaTheme.fallbackIcon("star")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.toggleFavorite(favoriteMenu.targetPath)
            }
        }
    }

    GlassContextMenu {
        id: deviceMenu
        backdropSource: root.backdropSource
        property string deviceName: ""

        GlassMenuItem {
            text: "Ocultar dispositivo"
            icon.name: "list-remove"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.hideDevice(deviceMenu.deviceName)
            }
        }

        GlassMenuItem {
            text: "Mostrar dispositivos ocultos"
            visible: root.hostWindow.activeController
                     && root.hostWindow.activeController.hiddenDeviceCount > 0
            height: visible ? implicitHeight : 0
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.unhideAllDevices()
            }
        }
    }
}
