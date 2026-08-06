import QtQuick
import org.celestina.fluorita 1.0

// Music: the tracks of the selected folder in the order the core projection
// already decided (artist → album → track), grouped by artist.
//
// The "Unknown artist" bucket is deliberate: an untagged track is still the
// user's music, and hiding it would be worse than admitting we do not know
// whose it is.
ListView {
    id: list

    required property FluoritaLibrary library
    // A track has no poster in this projection; the signature is the grid's so
    // the window has one door.
    signal activated(string path, rect origin, string poster, string kind)
    signal menuRequested(string path, string name, real x, real y)

    // Like the gallery: woven when the revision says every column of this
    // publication is in place.
    property var rows: []
    model: list.rows

    Connections {
        target: list.library
        function onRevisionChanged() { list.rows = list.weave(); }
    }

    Component.onCompleted: list.rows = list.weave()

    function weave() {
        var paths = list.library.musicPaths;
        var titles = list.library.musicTitles;
        var artists = list.library.musicArtists;
        var albums = list.library.musicAlbums;
        var live = list.library.musicAvailable;
        var covers = list.library.musicThumbnails;
        var count = Math.min(paths.length, titles.length, artists.length,
                             albums.length, live.length, covers.length);
        var woven = [];
        for (var index = 0; index < count; ++index) {
            woven.push({
                path: paths[index],
                title: titles[index],
                artist: artists[index],
                album: albums[index],
                available: live[index] === "1",
                thumbnail: covers[index]
            });
        }
        return woven;
    }

    clip: true
    focus: true
    // Joins the tab chain: without this the focus stays here and the header
    // actions cannot be reached without a pointer.
    activeFocusOnTab: true
    boundsBehavior: Flickable.StopAtBounds
    spacing: CelestinaTheme.spaceXs

    // The order already arrives grouped by artist, so sectioning reorders
    // nothing: it only puts the label where the artist changes.
    section.property: "artist"
    section.criteria: ViewSection.FullString
    section.delegate: CelestinaSectionLabel {
        required property string section

        width: ListView.view.width
        topPadding: CelestinaTheme.spaceMd
        text: section
    }

    Accessible.role: Accessible.List
    Accessible.name: qsTr("Música")

    delegate: Item {
        id: row

        required property var modelData
        required property int index

        width: ListView.view.width
        height: CelestinaTheme.rowHeight

        opacity: row.modelData.available ? 1 : CelestinaTheme.disabledContentOpacity

        Accessible.role: Accessible.ListItem
        Accessible.name: row.modelData.available
            ? row.modelData.title
            : qsTr("%1 — sin encontrar").arg(row.modelData.title)
        Accessible.description: qsTr("%1 · %2").arg(row.modelData.artist).arg(row.modelData.album)
        Accessible.focusable: true
        // The same four values the pointer sends; the path alone left the
        // origin, the cover and the kind undefined.
        Accessible.onPressAction: list.activated(row.modelData.path,
                                                 list.originOf(row),
                                                 row.modelData.thumbnail,
                                                 "audio")

        CelestinaSurface {
            anchors.fill: parent
            role: list.currentIndex === row.index
                ? CelestinaSurface.Selected
                : CelestinaSurface.Content

            contentItem: Row {
                spacing: CelestinaTheme.spaceMd

                CelestinaIcon {
                    anchors.verticalCenter: parent.verticalCenter
                    width: CelestinaTheme.iconMd
                    height: CelestinaTheme.iconMd
                    sourceSize: Qt.size(width, height)
                    name: "file-music"
                    fallbackName: "file"
                }

                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 0

                    Text {
                        text: row.modelData.title
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowTitle
                        elide: Text.ElideRight
                    }

                    Text {
                        text: row.modelData.album
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowSecondary
                        elide: Text.ElideRight
                    }
                }
            }
        }

        // One click plays it, for the same reason the grid opens on one click,
        // and through a MouseArea stacked above the surface for the same reason
        // the grid uses one: a `Pane` filling the row swallows the press that a
        // parent's TapHandler would need.
        MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: function(mouse) {
                list.currentIndex = row.index;
                list.forceActiveFocus();
                if (mouse.button === Qt.RightButton) {
                    const point = mapToItem(list, mouse.x, mouse.y);
                    list.menuRequested(row.modelData.path, row.modelData.title,
                                       point.x, point.y);
                    return;
                }
                list.activated(row.modelData.path, list.originOf(row),
                               row.modelData.thumbnail, "audio");
            }
        }
    }

    Keys.onReturnPressed: list.activateCurrent()
    Keys.onEnterPressed: list.activateCurrent()
    Keys.onSpacePressed: list.activateCurrent()

    Keys.onMenuPressed: list.menuForCurrent()

    function activateCurrent() {
        if (list.currentIndex < 0 || list.currentIndex >= list.model.length)
            return;
        const row = list.itemAtIndex(list.currentIndex);
        list.activated(list.model[list.currentIndex].path,
                       row ? list.originOf(row) : Qt.rect(0, 0, 0, 0),
                       list.model[list.currentIndex].thumbnail, "audio");
    }

    function originOf(row) {
        const point = row.mapToItem(null, 0, 0);
        return Qt.rect(point.x, point.y, row.width, row.height);
    }

    function menuForCurrent() {
        if (list.currentIndex < 0 || list.currentIndex >= list.model.length)
            return;
        const item = list.model[list.currentIndex];
        const row = list.itemAtIndex(list.currentIndex);
        const y = row ? row.y - list.contentY : 0;
        list.menuRequested(item.path, item.title, 0, y);
    }
}
