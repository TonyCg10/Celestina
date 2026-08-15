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
    // The same identity as a correctly escaped local URL. C++ owns path-to-URL
    // conversion so spaces and fragment characters remain filename data.
    required property url sourceUrl
    // The provider's exact file identity and output inventory generation. They
    // form part of the Image URL so replacing bytes at the same path cannot
    // leave Qt showing an older successful request.
    required property string sourceRevision
    required property double sourceGeneration
    required property int sourceWidth
    required property int sourceHeight
    required property string outputName
    required property bool reducedMotion

    // Captured only when the current Image request reaches Ready. Requested
    // identity is never treated as displayed identity merely because a
    // property changed.
    property string readySource: ""
    property string readyRevision: ""
    property double readyGeneration: 0
    property int readySourceWidth: 0
    property int readySourceHeight: 0

    readonly property url imageSource: wallpaper.sourceUrl.toString().length > 0
                                       ? wallpaper.sourceUrl.toString()
                                         + "#celestina-revision="
                                         + encodeURIComponent(wallpaper.sourceRevision)
                                         + "&celestina-generation="
                                         + wallpaper.sourceGeneration.toFixed(0)
                                         + "&celestina-geometry="
                                         + wallpaper.sourceWidth + "x"
                                         + wallpaper.sourceHeight
                                       : ""

    // A path this session cannot decode is the same as no path: the fallback
    // is painted rather than a broken-image placeholder or an empty frame.
    property bool decodable: true
    readonly property bool showingImage: wallpaper.source.length > 0
                                         && wallpaper.decodable
                                         && image.status === Image.Ready
                                         && wallpaper.readySource === wallpaper.source
                                         && wallpaper.readyRevision === wallpaper.sourceRevision
                                         && wallpaper.readyGeneration === wallpaper.sourceGeneration
                                         && wallpaper.readySourceWidth === wallpaper.sourceWidth
                                         && wallpaper.readySourceHeight === wallpaper.sourceHeight

    color: CelestinaTheme.compositorGlassFallback
    title: qsTr("Fondo de Celestina")

    Component.onCompleted: CelestinaTheme.reducedMotion = wallpaper.reducedMotion

    function invalidateReadyImage() {
        wallpaper.decodable = true;
        wallpaper.readySource = "";
        wallpaper.readyRevision = "";
        wallpaper.readyGeneration = 0;
        wallpaper.readySourceWidth = 0;
        wallpaper.readySourceHeight = 0;
    }

    onSourceChanged: wallpaper.invalidateReadyImage()
    onSourceUrlChanged: wallpaper.invalidateReadyImage()
    onSourceRevisionChanged: wallpaper.invalidateReadyImage()
    onSourceGenerationChanged: wallpaper.invalidateReadyImage()
    onSourceWidthChanged: wallpaper.invalidateReadyImage()
    onSourceHeightChanged: wallpaper.invalidateReadyImage()

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

        // The wallpaper the person was already looking at, held while the next
        // one decodes. Without it a change of image passes through a frame of
        // bare canvas: the loader above hides the moment the requested
        // identity changes, and an Image drops its old texture the moment its
        // source does — measured on the nest as one dark frame and a fade
        // from black on every wallpaper switch. This copy's source is only
        // ever assigned a file the loader finished showing, so it can never
        // resurrect a stale or unreadable request — an undecodable file still
        // falls through to the deliberate fallback, exactly as before.
        Image {
            id: retained

            anchors.fill: parent
            visible: source.toString().length > 0
                     && wallpaper.source.length > 0
                     && wallpaper.decodable
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: false
            sourceSize.width: wallpaper.sourceWidth > 0
                              ? wallpaper.sourceWidth : wallpaper.width
            sourceSize.height: wallpaper.sourceHeight > 0
                               ? wallpaper.sourceHeight : wallpaper.height
        }

        Image {
            id: image

            anchors.fill: parent
            visible: wallpaper.showingImage
            source: wallpaper.imageSource
            fillMode: Image.PreserveAspectCrop
            // A wallpaper is decoded once and looked at for hours; doing it off the
            // GUI thread keeps a large photograph from stalling the panel with it.
            asynchronous: true
            cache: false
            // Reading the file at the screen's own size rather than at the
            // photograph's: a 6000-pixel image would otherwise cost its full
            // decoded size in memory on every output showing it.
            sourceSize.width: wallpaper.sourceWidth > 0
                              ? wallpaper.sourceWidth : wallpaper.width
            sourceSize.height: wallpaper.sourceHeight > 0
                               ? wallpaper.sourceHeight : wallpaper.height

            onStatusChanged: {
                if (status === Image.Error) {
                    wallpaper.invalidateReadyImage();
                    wallpaper.decodable = false;
                } else if (status === Image.Ready
                           && image.source.toString()
                              === wallpaper.imageSource.toString()) {
                    wallpaper.readySource = wallpaper.source;
                    wallpaper.readyRevision = wallpaper.sourceRevision;
                    wallpaper.readyGeneration = wallpaper.sourceGeneration;
                    wallpaper.readySourceWidth = wallpaper.sourceWidth;
                    wallpaper.readySourceHeight = wallpaper.sourceHeight;
                    // What is on screen now is what the next change may keep
                    // showing while its replacement decodes.
                    retained.source = image.source;
                }
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
