import QtQuick
import org.celestina.siderita 1.0

// ─── FolderCellDelegate ─────────────────────────────────────────────────────
// Una celda de la rejilla: cuadro de selección centrado, glifo o miniatura
// grande y el nombre debajo; arrastre y suelte (spring-open sobre carpetas).
// Igual que la fila, pero para la cuadrícula. Los roles llegan del modelo; el
// panel, el controlador, la vista (cellWidth/Height + foco), la ventana, el
// fantasma de arrastre y dónde abrir menús se inyectan al instanciar el
// delegado; abrir en pestaña nueva y el menú contextual salen como señales.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    // Roles from the native SideritaEntryModel.
    required property int index
    required property string name
    required property string token
    required property string kind
    required property string subtitle
    required property string path
    required property bool isDirectory

    // Injected by the folder view when it declares the delegate.
    property var panel
    property var controller
    property var view
    property var hostWindow
    property Item ghost
    property Item overlayParent

    signal newTabRequested(string path, bool foreground)
    signal contextMenuRequested(string token, string name, bool isDir, string path, real x, real y)

    // Guarded against the brief construction window before the injected
    // panel/controller are set (they arrive from the view's delegate block).
    readonly property bool selected: root.panel ? root.panel.isSelected(token) : false
    readonly property bool hidden: name.charAt(0) === "."
    // Ghosted while cut (pending move); italic name distinguishes it from a
    // dimmed dotfile.
    readonly property bool cut: root.controller
                                ? root.controller.cutPaths.indexOf(path) >= 0 : false

    width: root.view.cellWidth
    height: root.view.cellHeight
    opacity: cut ? CelestinaTheme.unavailableContentOpacity
                 : hidden ? CelestinaTheme.disabledOpacity : 1
    Accessible.role: Accessible.ListItem
    Accessible.name: name
    Accessible.selected: selected

    // The selection square keeps its natural size and centres in the
    // (stretched-to-fill) cell, rather than ballooning to the full column width.
    Rectangle {
        anchors.centerIn: parent
        width: root.panel.gridCellWidth - 10
        height: parent.height - 10
        radius: CelestinaTheme.radiusSm
        color: root.selected
               ? CelestinaTheme.surfaceSelected
               : cellMouse.containsMouse
                 ? CelestinaTheme.surfaceHover
                 : CelestinaTheme.clear
        border.width: root.selected ? CelestinaTheme.borderHairline : 0
        border.color: CelestinaTheme.dividerStrong

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.motionFast
            }
        }
    }

    // Drop onto this cell when it is a folder (external file URLs or an
    // internal entry drag).
    DropArea {
        id: cellDrop
        anchors.fill: parent
        anchors.margins: 5
        enabled: root.isDirectory

        onEntered: function(drag) {
            if (!drag.hasUrls && !root.panel.isEntryDrag(drag)) {
                drag.accepted = false
                return
            }
            cellSpringOpen.restart()
        }
        onExited: cellSpringOpen.stop()
        onDropped: function(drop) {
            cellSpringOpen.stop()
            root.panel.dropOnto(root.path, drop)
            drop.accept()
        }

        // Spring-loaded, like the list rows.
        Timer {
            id: cellSpringOpen
            interval: CelestinaTheme.springDelay
            onTriggered: {
                if (cellDrop.containsDrag)
                    root.controller.openLocation(root.path)
            }
        }

        Rectangle {
            anchors.centerIn: parent
            width: root.panel.gridCellWidth - 10
            height: parent.height - 10
            visible: parent.containsDrag
            color: CelestinaTheme.clear
            radius: CelestinaTheme.radiusSm
            border.width: CelestinaTheme.borderFocus
            border.color: CelestinaTheme.accent
        }
    }

    Column {
        anchors.centerIn: parent
        spacing: 8

        Rectangle {
            id: cellGlyph
            anchors.horizontalCenter: parent.horizontalCenter
            width: Math.round(72 * root.hostWindow.contentIconScale)
            height: Math.round(72 * root.hostWindow.contentIconScale)
            radius: CelestinaTheme.radiusSm
            clip: true
            // La selección pertenece a la celda. Una segunda baldosa de color
            // detrás del icono producía dos cajas de selección enfrentadas.
            color: CelestinaTheme.clear

            readonly property string media: root.kind === "directory"
                                            ? "" : root.panel.mediaKind(root.name)

            CelestinaIcon {
                anchors.centerIn: parent
                visible: !cellThumb.ready
                width: Math.round(54 * root.hostWindow.contentIconScale)
                height: Math.round(54 * root.hostWindow.contentIconScale)
                name: root.panel.mediaIconName(root.kind, cellGlyph.media, root.path)
                sourceSize: Qt.size(width, height)
                fallbackName: root.kind === "directory"
                              ? "folder"
                              : root.kind === "symlink"
                                ? "symlink"
                                : "file"
                tone: root.panel.entryIconTone(root.kind)
                tintOverride: root.panel.iconTint(root.path)
            }

            Image {
                id: cellThumb
                anchors.fill: parent
                anchors.margins: 1
                readonly property bool ready: cellGlyph.media !== ""
                                              && status === Image.Ready
                visible: opacity > 0
                opacity: ready ? 1 : 0
                Behavior on opacity {
                    NumberAnimation { duration: CelestinaTheme.motionNormal }
                }
                source: cellGlyph.media !== ""
                        ? "image://thumb/" + encodeURIComponent(root.path) : ""
                sourceSize.width: 256
                sourceSize.height: 256
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: true
                smooth: true
            }

            FavoriteBadge {
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: 2
                iconScale: root.hostWindow.contentIconScale
                starred: root.panel.isFavorite(root.path)
            }

            // Play badge on a video frame.
            Rectangle {
                visible: cellThumb.ready && cellGlyph.media === "video"
                anchors.centerIn: parent
                width: Math.round(parent.width * 0.4)
                height: width
                radius: width / 2
                color: CelestinaTheme.mediaScrim
                CelestinaIcon {
                    anchors.centerIn: parent
                    width: Math.round(parent.width * 0.55)
                    height: width
                    name: "media-playback-start"
                    fallbackName: "media-play"
                    tone: CelestinaIcon.Overlay
                }
            }
        }

        Text {
            width: root.panel.gridCellWidth - 22
            horizontalAlignment: Text.AlignHCenter
            text: root.name
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale)
            font.italic: root.cut
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
                if (root.isDirectory)
                    root.newTabRequested(root.path, false)
                return
            }
            root.view.forceActiveFocus()
            root.view.currentIndex = root.index
            if (mouse.button === Qt.RightButton) {
                if (!root.panel.isSelected(root.token))
                    root.panel.selectOnly(root.token)
            } else if (mouse.modifiers & Qt.ControlModifier) {
                root.panel.toggleSelection(root.token)
            } else if (mouse.modifiers & Qt.ShiftModifier) {
                root.panel.selectRange(root.index)
            } else {
                root.panel.selectOnly(root.token)
            }
            root.controller.selectToken(root.token)
            if (mouse.button === Qt.RightButton) {
                const point = root.mapToItem(
                                root.overlayParent,
                                mouse.x, mouse.y)
                root.contextMenuRequested(root.token, root.name,
                                          root.isDirectory, root.path,
                                          point.x, point.y)
            }
        }

        onDoubleClicked: function(mouse) {
            if (mouse.button === Qt.LeftButton)
                root.controller.activateToken(root.token)
        }
    }

    DragHandler {
        id: cellDrag
        target: null
        dragThreshold: 8
        enabled: true
        onActiveChanged: {
            if (active)
                root.panel.startEntryDrag(
                    root.path, root.name, root.isDirectory, cellGlyph, cellDrag)
            else {
                root.ghost.Drag.drop()
                root.ghost.Drag.active = false
            }
        }
    }
}
