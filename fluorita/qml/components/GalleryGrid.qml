import QtQuick
import org.celestina.fluorita 1.0

// Galería: imágenes y vídeo juntos, como manda el contrato del producto.
//
// Las miniaturas salen del caché freedesktop compartido *si ya existen*. Nadie
// las genera aquí: hacerlo arrancaría un decodificador por tarjeta, que es
// justo el coste que navegar no debe pagar. Sin miniatura hay glifo, no hueco.
GridView {
    id: grid

    required property FluoritaLibrary library
    // Emitida al activar una tarjeta; la ventana decide qué hacer con ella.
    signal activated(string path)

    // Las columnas llegan alineadas por índice, pero se publican una a una: si
    // el modelo se atara a ellas, se reconstruiría a mitad de la publicación
    // con la mitad de las columnas del escaneo anterior. Por eso se teje una
    // sola vez, cuando la revisión dice que ya están todas.
    property var rows: []
    model: grid.rows

    Connections {
        target: grid.library
        function onRevisionChanged() { grid.rows = grid.weave(); }
    }

    Component.onCompleted: grid.rows = grid.weave()

    function weave() {
        var paths = grid.library.galleryPaths;
        var names = grid.library.galleryNames;
        var kinds = grid.library.galleryKinds;
        var thumbs = grid.library.galleryThumbnails;
        // Defensivo: una columna corta significaría un error de publicación, y
        // es mejor mostrar menos filas que filas con campos indefinidos.
        var count = Math.min(paths.length, names.length, kinds.length, thumbs.length);
        var woven = [];
        for (var index = 0; index < count; ++index) {
            woven.push({
                path: paths[index],
                name: names[index],
                kind: kinds[index],
                thumbnail: thumbs[index]
            });
        }
        return woven;
    }

    readonly property int columnTarget: Math.max(2, Math.floor(width / 190))

    cellWidth: Math.floor(width / grid.columnTarget)
    cellHeight: grid.cellWidth
    clip: true
    focus: true
    // Entra en la cadena de tabulación: sin esto el foco se queda aquí y las
    // acciones de la cabecera no se pueden alcanzar sin ratón.
    activeFocusOnTab: true
    // Sin scroll animado: el movimiento reducido no tiene nada que apagar aquí.
    boundsBehavior: Flickable.StopAtBounds

    Accessible.role: Accessible.List
    Accessible.name: qsTr("Galería")

    delegate: Item {
        id: cell

        required property var modelData
        required property int index

        width: grid.cellWidth
        height: grid.cellHeight

        Accessible.role: Accessible.Cell
        Accessible.name: cell.modelData.name
        Accessible.description: cell.modelData.kind
        Accessible.focusable: true
        Accessible.onPressAction: grid.activated(cell.modelData.path)

        CelestinaSurface {
            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceXs
            role: grid.currentIndex === cell.index
                ? CelestinaSurface.Selected
                : CelestinaSurface.Grouped

            contentItem: Item {
                Image {
                    id: thumbnail

                    anchors.fill: parent
                    anchors.bottomMargin: CelestinaTheme.spaceLg
                    source: cell.modelData.thumbnail
                    visible: cell.modelData.thumbnail.length > 0
                        && thumbnail.status === Image.Ready
                    asynchronous: true
                    autoTransform: true
                    fillMode: Image.PreserveAspectCrop
                    sourceSize.width: grid.cellWidth
                    sourceSize.height: grid.cellHeight
                }

                // Sin miniatura cacheada: el tipo, dicho con el icono del tema.
                CelestinaIcon {
                    anchors.centerIn: thumbnail
                    visible: !thumbnail.visible
                    width: Math.round(grid.cellWidth * 0.32)
                    height: width
                    sourceSize: Qt.size(width, height)
                    name: cell.modelData.kind === "vídeo" ? "file-video-camera" : "file-image"
                    fallbackName: "file"
                }

                Text {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    text: cell.modelData.name
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    elide: Text.ElideMiddle
                    horizontalAlignment: Text.AlignHCenter
                }
            }
        }

        TapHandler {
            acceptedButtons: Qt.LeftButton
            onSingleTapped: {
                grid.currentIndex = cell.index;
                grid.forceActiveFocus();
            }
            onDoubleTapped: grid.activated(cell.modelData.path)
        }
    }

    // El teclado abre lo que el ratón abre con doble clic.
    Keys.onReturnPressed: grid.activateCurrent()
    Keys.onEnterPressed: grid.activateCurrent()
    Keys.onSpacePressed: grid.activateCurrent()

    function activateCurrent() {
        if (grid.currentIndex >= 0 && grid.currentIndex < grid.model.length)
            grid.activated(grid.model[grid.currentIndex].path);
    }
}
