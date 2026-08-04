import QtQuick
import org.celestina.fluorita 1.0

// Gallery: the images and video of the selected folder, together.
//
// Thumbnails come from the shared freedesktop cache *only if they already
// exist*. Nothing generates one here: that would start a decoder per card,
// which is exactly the cost browsing must not pay. No thumbnail means a glyph,
// not a hole.
GridView {
    id: grid

    required property FluoritaLibrary library
    // Emitted when a card is activated; the window decides what to do with it.
    signal activated(string path)

    // The columns arrive index-aligned but are published one at a time: a model
    // bound to them would rebuild halfway through a publication with half the
    // columns still holding the previous one. So it is woven once, when the
    // revision says every column is in place.
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
        var live = grid.library.galleryAvailable;
        // Defensive: a short column would mean a publication error, and fewer
        // rows are better than rows with undefined fields.
        var count = Math.min(paths.length, names.length, kinds.length,
                             thumbs.length, live.length);
        var woven = [];
        for (var index = 0; index < count; ++index) {
            woven.push({
                path: paths[index],
                name: names[index],
                kind: kinds[index],
                thumbnail: thumbs[index],
                available: live[index] === "1"
            });
        }
        return woven;
    }

    readonly property int columnTarget: Math.max(2, Math.floor(width / 190))

    cellWidth: Math.floor(width / grid.columnTarget)
    cellHeight: grid.cellWidth
    clip: true
    focus: true
    // Joins the tab chain: without this the focus stays here and the header
    // actions cannot be reached without a pointer.
    activeFocusOnTab: true
    // No animated scrolling: reduced motion has nothing to switch off here.
    boundsBehavior: Flickable.StopAtBounds

    Accessible.role: Accessible.List
    Accessible.name: qsTr("Gallery")

    delegate: Item {
        id: cell

        required property var modelData
        required property int index

        width: grid.cellWidth
        height: grid.cellHeight

        // Dimmed, not hidden: the file is not where the catalogue saw it, and
        // making it disappear would read as data loss.
        opacity: cell.modelData.available ? 1 : CelestinaTheme.disabledContentOpacity

        Accessible.role: Accessible.Cell
        Accessible.name: cell.modelData.name
        Accessible.description: cell.modelData.available
            ? cell.modelData.kind
            : qsTr("%1 — not found").arg(cell.modelData.kind)
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

                // No cached thumbnail: the kind, said with the theme's icon.
                CelestinaIcon {
                    anchors.centerIn: thumbnail
                    visible: !thumbnail.visible
                    width: Math.round(grid.cellWidth * 0.32)
                    height: width
                    sourceSize: Qt.size(width, height)
                    name: cell.modelData.kind === "video" ? "file-video-camera" : "file-image"
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

        // One click opens it. Selecting on the first click and opening only on
        // the second made the library read as if activation were broken: the
        // card visibly responded and then nothing happened. A double click
        // still delivers two clicks, so the window ignores a request to open
        // what is already open rather than restarting the session.
        //
        // A MouseArea stacked above the card, not a handler on the item beneath
        // it. `CelestinaSurface` is a `Pane`, and a Control filling the cell
        // takes the press before a parent's TapHandler ever sees it — which is
        // why clicking a card did nothing at all while the keyboard worked.
        MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                grid.currentIndex = cell.index;
                grid.forceActiveFocus();
                grid.activated(cell.modelData.path);
            }
        }
    }

    // The keyboard opens what the pointer opens.
    Keys.onReturnPressed: grid.activateCurrent()
    Keys.onEnterPressed: grid.activateCurrent()
    Keys.onSpacePressed: grid.activateCurrent()

    function activateCurrent() {
        if (grid.currentIndex >= 0 && grid.currentIndex < grid.model.length)
            grid.activated(grid.model[grid.currentIndex].path);
    }
}
