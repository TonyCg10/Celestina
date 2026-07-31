import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import org.celestina.siderita 1.0

// ─── FolderRowDelegate ──────────────────────────────────────────────────────
// Una fila de la lista/detalles: cuadro de selección, glifo o miniatura, cuerpo
// (nombre+subtítulo, o columnas nombre/tamaño/fecha/tipo), arrastre y suelte
// (spring-open sobre carpetas). Los roles llegan del modelo; el panel
// (selección/medios/arrastre), el controlador, la vista (modo detalles + anchos
// de columna + foco), la ventana, el fantasma de arrastre y dónde abrir menús se
// inyectan al instanciar el delegado. Abrir en pestaña nueva y el menú contextual
// salen como señales — el delegado no alcanza ids externos.
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
    required property string sizeText
    required property string dateText

    // Injected by the folder view when it declares the delegate.
    property var panel
    property var controller
    property var view
    property var hostWindow
    property Item ghost
    property Item overlayParent

    signal newTabRequested(string path, bool foreground)
    signal contextMenuRequested(string token, string name, bool isDir, string path, real x, real y)

    // Guarded against the brief window during delegate construction where the
    // injected panel/controller are not set yet (they arrive from the view's
    // delegate block); once set these re-evaluate to the real state.
    readonly property bool selected: root.panel ? root.panel.isSelected(token) : false
    // Hidden (dotfile) entries are dimmed so they read as a distinct,
    // secondary block.
    readonly property bool hidden: name.charAt(0) === "."
    // Ghosted while it sits on the clipboard as a cut (pending move); an
    // italic name tells it apart from a mere dotfile.
    readonly property bool cut: root.controller
                                ? root.controller.cutPaths.indexOf(path) >= 0 : false

    width: root.view.width
    height: root.panel.listRowHeight
    opacity: cut ? CelestinaTheme.unavailableContentOpacity
                 : hidden ? CelestinaTheme.disabledOpacity : 1
    Accessible.role: Accessible.ListItem
    Accessible.name: name
    Accessible.selected: selected

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 4
        anchors.rightMargin: 4
        radius: CelestinaTheme.radiusSm
        color: root.selected
               ? CelestinaTheme.badgeAccentFill
               : pointer.containsMouse
                 ? CelestinaTheme.surfaceHover
                 : CelestinaTheme.clear
        border.width: 0
        border.color: CelestinaTheme.divider

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.motionFast
            }
        }
    }

    Rectangle {
        visible: root.selected
        x: 4
        anchors.verticalCenter: parent.verticalCenter
        width: CelestinaTheme.compSelectionIndicatorWidth
        height: CelestinaTheme.compSelectionIndicatorHeight
        radius: width / 2
        color: CelestinaTheme.accent
    }

    Rectangle {
        anchors.left: kindGlyph.right
        anchors.leftMargin: 12
        anchors.right: parent.right
        anchors.rightMargin: 16
        anchors.bottom: parent.bottom
        height: CelestinaTheme.borderHairline
        color: CelestinaTheme.divider
        visible: root.index < root.view.count - 1
    }

    // Drop onto this row when it is a folder → the drop lands inside that
    // folder. Accepts external file URLs and internal entry drags (move a
    // file/folder into this folder).
    DropArea {
        id: rowDrop
        anchors.fill: parent
        enabled: root.isDirectory

        onEntered: function(drag) {
            if (!drag.hasUrls && !root.panel.isEntryDrag(drag)) {
                drag.accepted = false
                return
            }
            springOpen.restart()
        }
        onExited: springOpen.stop()
        onDropped: function(drop) {
            springOpen.stop()
            root.panel.dropOnto(root.path, drop)
            drop.accept()
        }

        // Spring-loaded: hold a drag over a folder and it opens, so a move
        // into somewhere deep does not mean dropping it here first and
        // picking it up again.
        Timer {
            id: springOpen
            interval: CelestinaTheme.springDelay
            onTriggered: {
                if (rowDrop.containsDrag)
                    root.controller.openLocation(root.path)
            }
        }

        Rectangle {
            anchors.fill: parent
            anchors.leftMargin: 4
            anchors.rightMargin: 4
            visible: parent.containsDrag
            color: CelestinaTheme.clear
            radius: CelestinaTheme.radiusSm
            border.width: CelestinaTheme.borderFocus
            border.color: CelestinaTheme.accent
        }
    }

    Rectangle {
        id: kindGlyph
        x: 14
        anchors.verticalCenter: parent.verticalCenter
        width: Math.round(CelestinaTheme.glyphTile * root.hostWindow.contentIconScale)
        height: Math.round(CelestinaTheme.glyphTile * root.hostWindow.contentIconScale)
        radius: CelestinaTheme.radiusSm
        // La fila ya posee el hover y la selección. El fondo del glifo queda
        // transparente para no inventar un segundo contenedor de estado.
        color: CelestinaTheme.clear
        clip: true

        readonly property string media: root.kind === "directory"
                                        ? "" : root.panel.mediaKind(root.name)

        CelestinaIcon {
            anchors.centerIn: parent
            visible: !thumb.ready
            width: Math.round(CelestinaTheme.iconMd * root.hostWindow.contentIconScale)
            height: Math.round(CelestinaTheme.iconMd * root.hostWindow.contentIconScale)
            name: root.panel.mediaIconName(root.kind, kindGlyph.media, root.path)
            // El icono elegido a mano suele ser simbólico, y los simbólicos
            // sólo se publican a 16 px: sin pedir el tamaño explícito se dibujan
            // diminutos dentro de una celda hecha para una carpeta de 54.
            sourceSize: Qt.size(width, height)
            fallbackName: root.kind === "directory"
                          ? "folder"
                          : root.kind === "symlink"
                            ? "symlink"
                            : "file"
            tone: root.panel.entryIconTone(root.kind)
            tintOverride: root.panel.iconTint(root.path)
        }

        // The cached image / video-frame / cover the "thumb" provider
        // returns, covering the tile once decoded; the Lucide glyph shows
        // until then (or forever, for media the cache has no thumbnail of).
        Image {
            id: thumb
            anchors.fill: parent
            anchors.margins: 1
            readonly property bool ready: kindGlyph.media !== ""
                                          && status === Image.Ready
            // Fades up as it decodes, so a folder of photos fills in instead
            // of flickering glyph→picture.
            visible: opacity > 0
            opacity: ready ? 1 : 0
            Behavior on opacity {
                NumberAnimation { duration: CelestinaTheme.motionNormal }
            }
            source: kindGlyph.media !== ""
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
            anchors.margins: 1
            diameter: Math.round(13 * root.hostWindow.contentIconScale)
            starred: root.panel.isFavorite(root.path)
        }

        // A small play badge marks a video's frame apart from a still image.
        Rectangle {
            visible: thumb.ready && kindGlyph.media === "video"
            anchors.centerIn: parent
            width: Math.round(parent.width * 0.42)
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

    // List / search body: name over the combined subtitle.
    Column {
        id: rowText
        visible: !root.view.detailsMode
        x: kindGlyph.x + kindGlyph.width + 12
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width - x - 24
        spacing: 1

        Text {
            width: parent.width
            text: root.name
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.contentTextScale)
            font.weight: CelestinaTheme.weightMedium
            font.italic: root.cut
            elide: Text.ElideMiddle
        }

        Text {
            width: parent.width
            text: root.subtitle
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale)
            elide: Text.ElideRight
        }
    }

    // Details body: name (fills) · size · date · type, aligned to the
    // header's columns.
    RowLayout {
        visible: root.view.detailsMode
        x: root.view.detailsNameX
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width - x - 16
        spacing: 12

        Text {
            Layout.fillWidth: true
            text: root.name
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.contentTextScale)
            font.weight: CelestinaTheme.weightMedium
            font.italic: root.cut
            elide: Text.ElideMiddle
        }
        Text {
            Layout.preferredWidth: root.view.colSizeW
            horizontalAlignment: Text.AlignRight
            text: root.sizeText
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale)
            elide: Text.ElideRight
        }
        Text {
            Layout.preferredWidth: root.view.colDateW
            text: root.dateText
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale)
            elide: Text.ElideRight
        }
        Text {
            Layout.preferredWidth: root.view.colTypeW
            text: root.kind === "directory" ? "Carpeta"
                  : root.kind === "symlink" ? "Enlace" : "Archivo"
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.hostWindow.contentTextScale)
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
                root.Window.window.activateEntry(root.controller, root.token)
        }
    }

    DragHandler {
        id: rowDrag
        target: null
        dragThreshold: 8
        // Any entry is draggable (a file to move onto a folder, a folder to
        // move or to bookmark on the sidebar).
        enabled: true
        onActiveChanged: {
            if (active)
                root.panel.startEntryDrag(
                    root.path, root.name, root.isDirectory, kindGlyph, rowDrag)
            else {
                root.ghost.Drag.drop()
                root.ghost.Drag.active = false
            }
        }
    }
}
