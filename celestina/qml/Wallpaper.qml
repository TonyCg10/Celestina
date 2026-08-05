// One output's background.
//
// It paints exactly one of two things, and never a third that looks like
// either: the image chosen for this screen, or a deliberate fallback. There is
// no in-between state where an unreadable file leaves a black rectangle that a
// person could mistake for a very dark photograph — a failed decode falls back
// the same way an absent file does, and says so to assistive technology.
//
// Nothing here chooses the image. The shell's provider decides which file
// belongs to which output; this window is handed a path or nothing at all.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: wallpaper

    // The absolute path chosen for this output, or an empty string when the
    // provider says there is nothing to show here.
    required property string source
    required property string outputName
    required property bool reducedMotion

    // A path this session cannot decode is the same as no path: the fallback
    // is painted rather than a broken-image placeholder or an empty frame.
    property bool decodable: true
    readonly property bool showingImage: wallpaper.source.length > 0 && wallpaper.decodable

    color: CelestinaTheme.compositorGlassFallback
    title: qsTr("Fondo de Celestina")

    Component.onCompleted: CelestinaTheme.reducedMotion = wallpaper.reducedMotion

    onSourceChanged: wallpaper.decodable = true

    // The description belongs to an Item, not to this Window: Qt attaches
    // `Accessible` only to something deriving from Item or Action, and a live
    // session logged the rejection twice at startup. The surface still needs a
    // name — a screen reader meeting an unlabelled full-screen graphic has
    // nothing to say about it — so it hangs on the content instead.
    Item {
        id: scene

        anchors.fill: parent

        Accessible.role: Accessible.Graphic
        Accessible.name: wallpaper.showingImage
                         ? qsTr("Fondo en %1").arg(wallpaper.outputName)
                         : qsTr("Sin fondo en %1").arg(wallpaper.outputName)

        Image {
            id: image

            anchors.fill: parent
            visible: wallpaper.showingImage
            source: wallpaper.source.length > 0 ? "file://" + wallpaper.source : ""
            fillMode: Image.PreserveAspectCrop
            // A wallpaper is decoded once and looked at for hours; doing it off the
            // GUI thread keeps a large photograph from stalling the panel with it.
            asynchronous: true
            cache: false
            // Reading the file at the screen's own size rather than at the
            // photograph's: a 6000-pixel image would otherwise cost its full
            // decoded size in memory on every output showing it.
            sourceSize.width: wallpaper.width
            sourceSize.height: wallpaper.height

            onStatusChanged: {
                if (status === Image.Error)
                    wallpaper.decodable = false;
            }

            // Appearing is worth a fade; reduced motion keeps the image and drops
            // the travel.
            opacity: status === Image.Ready ? 1 : 0
            Behavior on opacity {
                enabled: !CelestinaTheme.reducedMotion
                NumberAnimation {
                    duration: CelestinaTheme.motionNormal
                    easing.type: CelestinaTheme.easeStandard
                }
            }
        }
    }
}
