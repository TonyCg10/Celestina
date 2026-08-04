import QtQuick
import org.celestina.fluorita 1.0

// What else is in the selected folder, and where the open item sits among it.
//
// Non-visual on purpose. Two surfaces need this and neither should own it: the
// filmstrip a photograph gets, and the side arrows a video or a track gets.
// Weaving the rows twice would mean two answers to "what is next" the first
// time a projection changed shape.
Item {
    id: navigator

    required property FluoritaLibrary library
    required property string currentPath

    // Everything the selected folder holds, gallery first and then tracks, in
    // the order each projection already decided.
    property var rows: []

    readonly property int index: navigator.indexOf(navigator.currentPath)
    readonly property bool hasPrevious: navigator.index > 0
    readonly property bool hasNext: navigator.index >= 0
        && navigator.index < navigator.rows.length - 1
    // One item is not a folder to move around in.
    readonly property bool navigable: navigator.rows.length > 1

    Connections {
        target: navigator.library
        function onRevisionChanged() { navigator.rows = navigator.weave(); }
    }

    Component.onCompleted: navigator.rows = navigator.weave()

    function weave() {
        var woven = [];
        var paths = navigator.library.galleryPaths;
        var names = navigator.library.galleryNames;
        var kinds = navigator.library.galleryKinds;
        var thumbs = navigator.library.galleryThumbnails;
        // Defensive: a short column would mean a publication error, and fewer
        // rows are better than rows with undefined fields.
        var count = Math.min(paths.length, names.length, kinds.length, thumbs.length);
        for (var index = 0; index < count; ++index) {
            woven.push({
                path: paths[index],
                name: names[index],
                kind: kinds[index],
                thumbnail: thumbs[index]
            });
        }
        var trackPaths = navigator.library.musicPaths;
        var titles = navigator.library.musicTitles;
        var covers = navigator.library.musicThumbnails;
        var tracks = Math.min(trackPaths.length, titles.length, covers.length);
        for (var track = 0; track < tracks; ++track) {
            woven.push({
                path: trackPaths[track],
                name: titles[track],
                kind: "audio",
                thumbnail: covers[track]
            });
        }
        return woven;
    }

    function indexOf(path) {
        for (var index = 0; index < navigator.rows.length; ++index) {
            if (navigator.rows[index].path === path)
                return index;
        }
        return -1;
    }

    // The neighbour `step` away, or undefined at either end. Deliberately not
    // wrapping: arriving back at the first photograph after the last one reads
    // as the application having lost your place.
    function neighbour(step) {
        const at = navigator.index;
        if (at < 0)
            return undefined;
        const wanted = at + step;
        if (wanted < 0 || wanted >= navigator.rows.length)
            return undefined;
        return navigator.rows[wanted];
    }
}
