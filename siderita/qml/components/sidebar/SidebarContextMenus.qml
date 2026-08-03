import QtQuick
import org.celestina.siderita 1.0

Item {
    id: root

    required property var hostWindow
    required property Item overlayParent
    required property Item backdropSource
    property int bookmarkCount: 0

    signal editBookmarkRequested(int index)

    // El estado que gobierna "Abrir en pestaña nueva" de un dispositivo, visible
    // para quien lo prueba: sin montar no hay ruta y la acción no se ofrece.
    readonly property string deviceMountPoint: deviceMenu.mountPoint
    readonly property bool deviceCanOpenTab: deviceMenu.mountPoint.length > 0
    readonly property string phoneMountPath: phoneMenu.mountPath

    // Qué ruta llevará "Propiedades" en cada menú, y por tanto si se ofrece. Un
    // lugar virtual —Recientes, Papelera— y un volumen sin montar no tienen
    // ruta en disco, así que ahí no hay propiedades que enseñar.
    readonly property string placeTargetPath: placeMenu.targetPath
    readonly property string favoriteTargetPath: favoriteMenu.targetPath
    readonly property string bookmarkTargetPath: bookmarkMenu.targetPath

    function closeAll() {
        bookmarkMenu.close()
        placeMenu.close()
        favoriteMenu.close()
        deviceMenu.close()
        phoneMenu.close()
    }

    function openBookmark(index, path, point) {
        bookmarkMenu.targetIndex = index
        bookmarkMenu.targetPath = path
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

    function openDevice(name, mountPoint, point) {
        deviceMenu.deviceName = name
        deviceMenu.mountPoint = mountPoint
        deviceMenu.popup(overlayParent, point)
    }

    // El móvil no tenía menú: no se podía abrir en otra pestaña ni con el botón
    // central ni por menú, que es justo lo que faltaba.
    function openPhone(mountPath, point) {
        phoneMenu.mountPath = mountPath
        phoneMenu.popup(overlayParent, point)
    }

    GlassContextMenu {
        id: bookmarkMenu
        backdropSource: root.backdropSource
        property int targetIndex: -1
        property string targetPath: ""

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
            text: "Propiedades"
            visible: bookmarkMenu.targetPath.length > 0
            height: visible ? implicitHeight : 0
            icon.name: "document-properties"
            icon.source: CelestinaTheme.fallbackIcon("info")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.openProperties(bookmarkMenu.targetPath)
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
            text: "Propiedades"
            visible: placeMenu.targetPath.length > 0
            height: visible ? implicitHeight : 0
            icon.name: "document-properties"
            icon.source: CelestinaTheme.fallbackIcon("info")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.openProperties(placeMenu.targetPath)
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
            text: "Propiedades"
            icon.name: "document-properties"
            icon.source: CelestinaTheme.fallbackIcon("info")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.openProperties(favoriteMenu.targetPath)
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
        property string mountPoint: ""

        GlassMenuItem {
            text: "Abrir en pestaña nueva"
            // Un volumen sin montar todavía no tiene ruta, y montarlo es un
            // trabajo asíncrono: se ofrece cuando de verdad se puede cumplir.
            enabled: deviceMenu.mountPoint.length > 0
            icon.name: "tab-new"
            icon.source: CelestinaTheme.fallbackIcon("plus")
            onTriggered: root.hostWindow.openTab(deviceMenu.mountPoint, true)
        }

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
            text: "Propiedades"
            visible: deviceMenu.mountPoint.length > 0
            height: visible ? implicitHeight : 0
            icon.name: "document-properties"
            icon.source: CelestinaTheme.fallbackIcon("info")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.openProperties(deviceMenu.mountPoint)
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

    GlassContextMenu {
        id: phoneMenu
        backdropSource: root.backdropSource
        property string mountPath: ""

        GlassMenuItem {
            text: "Abrir en pestaña nueva"
            enabled: phoneMenu.mountPath.length > 0
            icon.name: "tab-new"
            icon.source: CelestinaTheme.fallbackIcon("plus")
            onTriggered: root.hostWindow.openTab(phoneMenu.mountPath, true)
        }
    }
}
