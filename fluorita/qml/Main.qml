import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.fluorita 1.0

// Fluorita's window, in two modes and no more: handed a file it is a player,
// launched bare it is a library. Activating something from the library is the
// same path as being handed it on the command line, so there is one way to
// start playing rather than two that can disagree.
ApplicationWindow {
    id: window

    required property bool reducedMotion
    // The item named on the command line, already reduced to a display label
    // by the Rust side. Empty means Fluorita was launched with no argument.
    required property string requestedLabel
    // Its classified kind, decided from the name alone — no decoder ran.
    required property string requestedKind
    // The real path, byte-exact, for the engine. Never rebuilt from the label.
    required property string requestedPath

    width: 960
    height: 640
    minimumWidth: 420
    minimumHeight: 320
    visible: true
    color: CelestinaTheme.canvas
    title: window.openLabel.length > 0
        ? qsTr("Fluorita — %1").arg(window.openLabel)
        : qsTr("Fluorita")

    // Opening an item: the player takes the window, growing out of whatever the
    // person clicked. The label is for display only; the path is what goes to
    // the engine.
    function open(path, origin, poster, kind) {
        // Opening what is already open would tear the session down and build it
        // again — a visible restart for anyone who double-clicks a card out of
        // habit now that one click is enough.
        if (path === window.openPath) {
            return;
        }
        // Only the first open of a session grows from a card. Stepping along
        // the dock replaces the item inside a frame that is already full-size,
        // and re-running the expansion for each step would be a lurch, not a
        // transition.
        if (!window.playing && origin !== undefined && origin.width > 0) {
            window.openOrigin = origin;
        }
        window.openPoster = poster === undefined ? "" : poster;
        window.openKind = kind === undefined ? window.requestedKind : kind;
        window.openPath = path;
        window.openLabel = path.substring(path.lastIndexOf("/") + 1);
        mediaPlayer.open(path);
    }

    // Leaving: the frame shrinks back to the card it came from and the session
    // closes when it lands. Closing first would shrink an empty rectangle.
    // Moving to a neighbour inside an already-open frame.
    function step(delta) {
        const wanted = folderNavigator.neighbour(delta);
        if (wanted === undefined) {
            return;
        }
        // No origin: the frame is already full size, and stepping must not
        // re-run the expansion.
        window.open(wanted.path, Qt.rect(0, 0, 0, 0), wanted.thumbnail, wanted.kind);
    }

    function backToLibrary() {
        if (!window.playing) {
            return;
        }
        // With motion off there is no animation to wait for, and waiting for
        // one that never runs would strand the window on a frame that will not
        // move.
        if (CelestinaTheme.reducedMotion) {
            window.libraryReached();
            return;
        }
        window.closing = true;
    }

    // The frame is dismantled only once the surface has let the backend go. The
    // renderer frees its context while its item is still in the scene graph, so
    // a frame that vanished first would strand a context nobody can release and
    // leave the core to be destroyed underneath it.
    function libraryReached() {
        window.pendingRelease = "library";
        mediaPlayer.close();
        if (mediaPlayer.renderHandle === 0) {
            // A track or a still never had a render surface: there is nothing
            // to wait for, and waiting would strand the frame instead.
            window.releaseSettled();
        }
    }

    // Whatever is waiting for the surface to let go: empty for nothing,
    // `library` for the return to the library, `close` for the window itself.
    property string pendingRelease: ""

    function releaseSettled() {
        const waiting = window.pendingRelease;
        // Stepping to the next item releases a surface too, and that is nobody's
        // cue to take the window apart.
        if (waiting === "") {
            return;
        }
        window.pendingRelease = "";
        if (waiting === "close") {
            window.closeAuthorised = true;
            window.close();
            return;
        }
        window.closing = false;
        window.openPath = "";
        window.openLabel = "";
    }

    // Empty until something is activated: the library is what a bare launch
    // shows, and the player takes over the window when there is an item.
    property string openPath: window.requestedPath
    property string openLabel: window.requestedLabel
    readonly property bool playing: window.openPath.length > 0
    // True while the frame is on its way back to the library.
    property bool closing: false
    // Where the open item came from. A bare launch has no card to grow out of,
    // so the frame starts at the window's own centre and the expansion reads as
    // the window settling rather than as a card that was never there.
    property rect openOrigin: Qt.rect(window.width / 2, window.height / 2, 0, 0)
    // The thumbnail the card was already showing. It is what grows, so opening
    // never shows black while a decoder starts, and closing shrinks a picture
    // rather than the hole left by a session that was torn down first.
    property string openPoster: ""
    // What is open, as the classification token the library published. It
    // decides which way the folder is navigated: a picture gets the filmstrip,
    // a video or a track gets arrows.
    property string openKind: window.requestedKind
    readonly property bool expanded: window.playing && !window.closing
    // The seek bar's own step, so the keyboard agrees with the control it is
    // standing in for rather than inventing a second idea of "a bit".
    readonly property int seekStep: 5

    FluoritaPlayer {
        id: mediaPlayer
    }

    FluoritaLibrary {
        id: mediaLibrary
    }

    CelestinaBackdrop {
        anchors.fill: parent
        visible: !window.playing || !playerSurface.showsPicture
    }

    LibraryView {
        anchors.fill: parent
        // Stays under the growing frame instead of disappearing the moment
        // something is activated: the card has to still be there for the frame
        // to look like it came out of it.
        visible: !window.playing || !window.expanded
        library: mediaLibrary
        // Activating an item is exactly what the command line does, so it goes
        // through the same door.
        onActivated: function(path, origin, poster, kind) {
            window.open(path, origin, poster, kind)
        }
    }

    // The frame the player lives in. Its geometry is the whole animation: it
    // starts as the card and ends as the window, and reverses on the way out.
    Item {
        id: playerFrame

        visible: window.playing
        clip: true

        x: window.expanded ? 0 : window.openOrigin.x
        y: window.expanded ? 0 : window.openOrigin.y
        width: window.expanded ? window.width : window.openOrigin.width
        height: window.expanded ? window.height : window.openOrigin.height

        // One behaviour per edge rather than a state machine: the frame has one
        // property that matters and two ends, and `expanded` already says which
        // end it is heading for.
        Behavior on x { NumberAnimation { duration: playerFrame.travel; easing.type: CelestinaTheme.easeStandard } }
        Behavior on y { NumberAnimation { duration: playerFrame.travel; easing.type: CelestinaTheme.easeStandard } }
        Behavior on width { NumberAnimation { duration: playerFrame.travel; easing.type: CelestinaTheme.easeStandard } }
        Behavior on height {
            NumberAnimation {
                duration: playerFrame.travel
                easing.type: CelestinaTheme.easeStandard
                // The frame's arrival is what ends the session, not a timer
                // that would race the animation on a slow frame. Height is as
                // good an edge as any: all four finish together.
                onRunningChanged: if (!running && window.closing) window.libraryReached()
            }
        }

        // Reduced motion has no scale animation to soften: it becomes instant.
        readonly property int travel: CelestinaTheme.reducedMotion
            ? 0 : CelestinaTheme.motionNormal

        PlayerSurface {
            id: playerSurface

            anchors.fill: parent
            player: mediaPlayer
            label: window.openLabel
            ambientSource: window.openPoster

            onReleased: window.releaseSettled()
        }

        // The card's own thumbnail, over the surface until the real picture is
        // up — and again for the whole way back.
        //
        // Fitted, exactly like `ImageView`, and that is the whole point: the
        // handoff is invisible only if both sides frame the picture the same
        // way. Cropping here to match the card instead cost seconds of a
        // wildly zoomed-in image, because a crop at window size scales the
        // picture to *cover* the window and the real one takes far longer to
        // decode than the 200 ms the growth lasts. The price is a thumbnail
        // that is letterboxed rather than cropped for that fifth of a second
        // at the start; the alternative was the defect.
        //
        // The handoff waits for a picture to actually be on screen rather than
        // merely requested: handing over when the player has a source only
        // moves the black from before the animation to after it.
        Image {
            id: travellingPoster

            anchors.fill: parent
            // The same inset `ImageView` uses for the transport. Without it the
            // handoff nudges the picture up by that margin, which is a small
            // jump exactly where the whole point is that there is none.
            anchors.bottomMargin: CelestinaTheme.spaceLg * 3
            source: window.openPoster
            visible: window.openPoster.length > 0
                && travellingPoster.status === Image.Ready
                && (window.closing || !playerSurface.picturePresented)
            asynchronous: false
            autoTransform: true
            fillMode: Image.PreserveAspectFit
            // Sized for the window it is heading for, not for the card it left,
            // so growing does not turn the picture to mush on the way.
            sourceSize.width: window.width
            sourceSize.height: window.height
        }

        // Where the rest of the folder is, and where we are in it. One owner
        // for both ways of moving through it.
        ContentNavigator {
            id: folderNavigator

            library: mediaLibrary
            currentPath: window.openPath
        }

        // A picture gets the filmstrip. It only appears once the frame has
        // arrived: a strip sliding around inside a growing rectangle would be
        // two transitions fighting.
        ContentDock {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            visible: window.expanded && window.openKind === "image"
            navigator: folderNavigator
            onActivated: function(path, origin, poster, kind) {
                window.open(path, origin, poster, kind)
            }
        }

        // A video or a track gets previous and next instead: a strip of posters
        // you cannot read at a glance, over a playing film, is furniture.
        ContentArrows {
            visible: window.expanded && window.openKind !== "image"
            navigator: folderNavigator
            onStepped: function(delta) { window.step(delta) }
        }
    }

    Shortcut {
        sequence: "Space"
        enabled: window.playing
        onActivated: mediaPlayer.toggle()
    }
    // Seeking everywhere in the window, for the same reason volume already is:
    // reaching for the arrow keys while watching something is not a request to
    // first find and focus a bar. The step is the seek bar's own, so the
    // keyboard moves the playhead by the same five seconds wherever it is
    // pressed.
    //
    // Held back while a picture is open, because a picture has nothing to seek
    // and its filmstrip owns Left/Right for stepping along the folder. A video
    // or a track gets arrows rather than a filmstrip, so nothing else wants
    // these keys.
    Shortcut {
        sequence: "Left"
        enabled: window.playing && mediaPlayer.timed && window.openKind !== "image"
        onActivated: mediaPlayer.seek(
            Math.max(0, mediaPlayer.positionSeconds - window.seekStep))
    }
    Shortcut {
        sequence: "Right"
        enabled: window.playing && mediaPlayer.timed && window.openKind !== "image"
        onActivated: mediaPlayer.seek(
            Math.min(mediaPlayer.durationSeconds,
                     mediaPlayer.positionSeconds + window.seekStep))
    }
    // Volume everywhere in the window, not only while the volume slider
    // happens to have focus — the seek bar takes focus first when playback
    // starts, and Up/Down reaching it as a second Left/Right would step the
    // playhead instead of the level a person actually meant to change.
    Shortcut {
        sequence: "Up"
        enabled: window.playing
        onActivated: mediaPlayer.setVolume(Math.min(1, mediaPlayer.volumeLevel + 0.05))
    }
    Shortcut {
        sequence: "Down"
        enabled: window.playing
        onActivated: mediaPlayer.setVolume(Math.max(0, mediaPlayer.volumeLevel - 0.05))
    }
    // Generate the missing thumbnails without depending on the pointer or on
    // the tab order.
    Shortcut {
        sequence: "Ctrl+G"
        enabled: !window.playing && mediaLibrary.artworkPending > 0
            && mediaLibrary.artworkState === "idle"
        onActivated: mediaLibrary.generateArtwork()
    }
    Shortcut {
        sequence: "Ctrl+Shift+G"
        enabled: !window.playing && mediaLibrary.artworkState === "generating"
        onActivated: mediaLibrary.cancelArtwork()
    }

    // Back to the library: closes the session and gives the window back to the
    // content, without leaving the application.
    Shortcut {
        sequence: "Escape"
        enabled: window.playing
        onActivated: window.backToLibrary()
    }
    Shortcut {
        sequences: [StandardKey.Close, StandardKey.Quit]
        onActivated: window.close()
    }

    // Closing while a session is open would drop the backend under the surface
    // still rendering from it: the player clears the handle first and the
    // window leaves once the surface has confirmed.
    property bool closeAuthorised: false

    onClosing: function(close) {
        if (window.closeAuthorised || mediaPlayer.renderHandle === 0) {
            close.accepted = true
            return
        }
        // Waited for, not scheduled. `Qt.callLater` closed the window on the
        // next event-loop pass whether or not the surface had answered, so the
        // core's destruction ran beside the scene graph's teardown.
        close.accepted = false
        window.pendingRelease = "close"
        mediaPlayer.close()
    }

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = window.reducedMotion
        if (window.requestedPath.length > 0) {
            mediaPlayer.open(window.requestedPath)
        } else {
            // No argument: the library. The scan runs on the engine's worker,
            // so this call returns at once.
            mediaLibrary.scan()
        }
    }
}
