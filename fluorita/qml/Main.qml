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
    // The item's path key: percent-encoded path bytes, byte-exact even for a
    // name that is not UTF-8, and the only thing the player accepts. Never
    // rebuilt from the label, and never shown.
    required property string requestedKey

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
    // person clicked. `key` is what goes to the engine; `name` is what a person
    // reads. The two arrive together because the row publishes both — deriving
    // the label from the key would put percent escapes in the title bar, and
    // deriving the key from the label would name a file that does not exist.
    function open(key, name, origin, poster, kind) {
        // Opening what is already open would tear the session down and build it
        // again — a visible restart for anyone who double-clicks a card out of
        // habit now that one click is enough.
        if (key === window.openKey) {
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
        // A glance ends the moment a person commits to the thing: the frame is
        // about to grow, and leaving the flag set would keep it card-sized over
        // a film that is now open.
        window.previewing = false;
        window.openKey = key;
        window.openLabel = name === undefined ? window.requestedLabel : name;
        mediaPlayer.open(key);
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
        window.open(wanted.key, wanted.name, Qt.rect(0, 0, 0, 0),
                    wanted.thumbnail, wanted.kind);
    }

    // What happens when something ends.
    //
    // The rule is the domain's; this only asks it, and only once the engine has
    // *confirmed* the end. Acting on a position near the duration instead would
    // skip a track whose last seconds failed to decode.
    Connections {
        target: mediaPlayer

        function onStateChanged() {
            if (mediaPlayer.state !== "terminado" || !window.playing) {
                return
            }
            const wanted = mediaPlayer.nextInFolder(folderNavigator.index,
                                                    folderNavigator.rows.length)
            if (wanted < 0) {
                return
            }
            const row = folderNavigator.rows[wanted]
            window.open(row.key, row.name, Qt.rect(0, 0, 0, 0), row.thumbnail, row.kind)
        }
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
        window.openKey = "";
        window.openLabel = "";
    }

    // Empty until something is activated: the library is what a bare launch
    // shows, and the player takes over the window when there is an item.
    property string openKey: window.requestedKey
    property string openLabel: window.requestedLabel
    readonly property bool playing: window.openKey.length > 0
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

    // A glance at a film under the pointer: where the frame sits while it
    // lasts, and whether one is running at all.
    property rect previewOrigin: Qt.rect(0, 0, 0, 0)
    property bool previewing: false
    // The seek bar's own step, so the keyboard agrees with the control it is
    // standing in for rather than inventing a second idea of "a bit".
    readonly property int seekStep: 5

    FluoritaPlayer {
        id: mediaPlayer
    }

    FluoritaLibrary {
        id: mediaLibrary
    }

    FluoritaEditor {
        id: mediaEditor
    }

    FluoritaMetadata {
        id: mediaMetadata
    }


    FluoritaBatch {
        id: mediaBatch
    }

    CelestinaBackdrop {
        anchors.fill: parent
        visible: !window.playing || !playerSurface.showsPicture
    }

    LibraryView {
        id: libraryView

        anchors.fill: parent
        // Stays under the growing frame instead of disappearing the moment
        // something is activated: the card has to still be there for the frame
        // to look like it came out of it.
        visible: !window.playing || !window.expanded
        library: mediaLibrary
        editor: mediaEditor
        metadata: mediaMetadata
        batch: mediaBatch
        // Activating an item is exactly what the command line does, so it goes
        // through the same door.
        onActivated: function(key, name, origin, poster, kind) {
            window.open(key, name, origin, poster, kind)
        }
        onEditRequested: function(key) { window.edit(key) }
        onMetadataRequested: function(key) { mediaMetadata.openItem(key) }

        // A glance at a film, in the frame the window already owns.
        //
        // The same surface the immersive view uses, put over the card and made
        // small: one video item in this window and one render context, which is
        // the only arrangement whose teardown this application has ever got
        // right. It is refused outright while something is open, because the
        // frame cannot be in two places.
        onPreviewRequested: function(key, origin) {
            if (window.playing) {
                return
            }
            window.previewOrigin = origin
            window.previewing = true
            mediaPlayer.preview(key)
        }
        onPreviewDropped: {
            if (!window.previewing) {
                return
            }
            window.previewing = false
            mediaPlayer.close()
        }
    }

    // The frame the player lives in. Its geometry is the whole animation: it
    // starts as the card and ends as the window, and reverses on the way out.
    Item {
        id: playerFrame

        visible: window.playing || window.previewing
        clip: true

        // Three places this frame can be: over the card it is growing from,
        // filling the window, or sitting on a card as a preview. The preview
        // never animates — a rectangle sliding between cards as the pointer
        // moves would be chasing it, not following it.
        x: window.previewing ? window.previewOrigin.x
            : window.expanded ? 0 : window.openOrigin.x
        y: window.previewing ? window.previewOrigin.y
            : window.expanded ? 0 : window.openOrigin.y
        width: window.previewing ? window.previewOrigin.width
            : window.expanded ? window.width : window.openOrigin.width
        height: window.previewing ? window.previewOrigin.height
            : window.expanded ? window.height : window.openOrigin.height

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
            itemKey: window.openKey
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
            currentKey: window.openKey
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
            onActivated: function(key, name, origin, poster, kind) {
                window.open(key, name, origin, poster, kind)
            }
        }

        // Looking closer, where the picture is. Beside the pencil because they
        // are the same kind of act on the same thing, and a person reaching for
        // one has usually just tried the other.
        Row {
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: CelestinaTheme.spaceLg
            spacing: CelestinaTheme.spaceXs
            visible: window.expanded && !mediaEditor.open && playerSurface.showsImage

            CelestinaIconButton {
                // The style has one magnifier and no second one with a minus in
                // it. Rather than invent an icon here — assets belong to
                // celestina-style, not to a consumer — the same glyph carries
                // the state: pressed means the picture is enlarged.
                iconName: "search"
                helpText: playerSurface.imageZoom.zoomed ? qsTr("Ver la imagen entera")
                                                         : qsTr("Acercar")
                checkable: true
                checked: playerSurface.imageZoom.zoomed
                onClicked: playerSurface.imageZoom.toggle()
            }

            CelestinaIconButton {
                visible: mediaEditor.admits(window.openKey)
                iconName: "pencil"
                helpText: qsTr("Editar esta imagen")
                onClicked: window.edit(window.openKey)
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

    // A refusal with nowhere to appear.
    //
    // Both surfaces write what happened into their own `notice`, and both only
    // show it while they are open — so a request they turned down before
    // opening said nothing at all. The menu no longer offers what an item does
    // not admit, but a file can still vanish or be too large between the click
    // and the read, and that has to be visible somewhere.
    Item {
        id: refusal

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        // Above the band the transport and the batch bar sit in, not on it:
        // the pill lets clicks through, but a notice that lands on the seek
        // bar still hides the control it lets them reach. Both bars are one
        // control row with the pill's own padding, inset by the same margin.
        anchors.bottomMargin: CelestinaTheme.spaceLg + CelestinaTheme.controlHeightXs
            + CelestinaTheme.spaceMd * 2 + CelestinaTheme.spaceMd
        width: notice.implicitWidth + CelestinaTheme.spaceLg * 2
        height: notice.implicitHeight + CelestinaTheme.spaceMd * 2
        visible: refusal.opacity > 0
        opacity: 0

        readonly property string message: !mediaEditor.open && mediaEditor.notice.length > 0
            ? mediaEditor.notice
            : !mediaMetadata.open && mediaMetadata.notice.length > 0
                ? mediaMetadata.notice
                : mediaPlayer.frameNotice.length > 0
                    ? mediaPlayer.frameNotice
                    : mediaBatch.notice.length > 0 && !mediaBatch.running
                        ? mediaBatch.notice
                        : ""

        onMessageChanged: if (refusal.message.length > 0) linger.restart()

        Behavior on opacity {
            NumberAnimation {
                duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionNormal
                easing.type: CelestinaTheme.easeStandard
            }
        }

        // Long enough to read a sentence, short enough that it does not become
        // furniture. It is a report, not a state.
        Timer {
            id: linger

            interval: 6000
            onTriggered: refusal.opacity = 0
            onRunningChanged: if (running) refusal.opacity = 1
        }

        GlassSurface {
            anchors.fill: parent
            cornerRadius: CelestinaTheme.radiusPill
        }

        CelestinaSectionLabel {
            id: notice

            anchors.centerIn: parent
            text: refusal.message
        }
    }

    // What a file says about itself, over whatever is open.
    MetadataPanel {
        anchors.fill: parent
        metadata: mediaMetadata
    }

    // Editing takes the window: the picture is the work, and the library
    // underneath it is not. It closes back to whatever was open before.
    EditSurface {
        anchors.fill: parent
        visible: mediaEditor.open
        focus: mediaEditor.open
        editor: mediaEditor
        onClosed: mediaEditor.close()
    }

    // Editing what is open, or what the pointer named, without going through
    // the menu.
    Shortcut {
        sequence: "Ctrl+E"
        enabled: !mediaEditor.open && window.playing
            && mediaEditor.admits(window.openKey)
        onActivated: window.edit(window.openKey)
    }

    // Measuring what the picture is doing, and keeping the measurement.
    //
    // Shortcuts rather than controls in the transport: this is a diagnostic for
    // the moment something looks wrong, not part of watching a film. Without
    // them the read-out in the player surface existed and nothing could ever
    // turn it on.
    Shortcut {
        sequence: "Ctrl+Shift+P"
        enabled: window.playing
        onActivated: mediaPlayer.togglePacing()
    }

    Shortcut {
        sequence: "Ctrl+Shift+S"
        enabled: window.playing && mediaPlayer.capturingPacing
        onActivated: mediaPlayer.writePacingReport()
    }

    // A `Shortcut` resolves before the focused item ever sees the key, so the
    // ones that share a key with an overlay stand down while it is open: the
    // editor's Escape leaves a tool before it leaves the editor and its text
    // prompt cancels on it; the metadata fields take Space as a letter.
    readonly property bool overlayOpen: mediaEditor.open || mediaMetadata.open

    Shortcut {
        sequence: "Space"
        enabled: window.playing && !window.overlayOpen
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
        enabled: window.playing && !window.overlayOpen
        onActivated: mediaPlayer.setVolume(Math.min(1, mediaPlayer.volumeLevel + 0.05))
    }
    Shortcut {
        sequence: "Down"
        enabled: window.playing && !window.overlayOpen
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
        enabled: window.playing && !window.overlayOpen
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

    // One door into the editor, wherever the request came from.
    function edit(key) {
        if (key.length === 0) {
            return
        }
        mediaEditor.openItem(key)
    }

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = window.reducedMotion
        if (window.requestedKey.length > 0) {
            mediaPlayer.open(window.requestedKey)
        } else {
            // No argument: the library. The scan runs on the engine's worker,
            // so this call returns at once.
            mediaLibrary.scan()
        }
    }
}
