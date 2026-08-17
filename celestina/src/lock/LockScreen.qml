// One output's cover, while the session is locked.
//
// What it shows is the whole of what a locked screen is allowed to know: the
// time, a place to type, and what happened to the last attempt. ADR 0004 is
// explicit about the rest and it is worth restating where the temptation
// lives — no notification bodies, no media titles, no clipboard, no window
// list. A lock screen that renders someone's messages has already failed at
// the one thing it exists for.
//
// The backdrop is this surface's own, and it has to be. `ext-session-lock`
// means the compositor has stopped showing the session, so there is nothing
// behind this to see through and nothing of the session to sample: what a
// cover paints, it paints itself. The one thing it may honestly paint is the
// wallpaper that output was already showing — the session's own look, visible
// to anyone standing in front of the screen a moment earlier, and not anyone's
// content. No window, panel or notification joins it.
//
// An output with no wallpaper, or one whose file will not decode, keeps the
// deliberate canvas. There is no in-between state where an unreadable file
// leaves a black rectangle a person could mistake for a very dark photograph.
//
// Locking then sets that backdrop back rather than replacing it: the wallpaper
// recedes a little and goes heavily out of focus, and the clock and prompt
// arrive above it. What the recession cannot include is the session itself —
// no panel, no window, no composited picture of the desktop — because the
// compositor stopped showing all of it before this surface existed. The
// wallpaper receding is the honest half of that gesture, and it is the whole
// of it.
//
// The blur is a cross-fade to a blurred copy rather than an animated radius.
// Animating a full-screen blur re-runs the whole downsample pyramid every
// frame; the copy is rendered from a still image and only its opacity travels,
// which is the same picture for a fraction of the GPU work. On this surface
// that is not only a performance note — a lock that stalls its own renderer
// leaves the session covered with nothing to type into.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Effects
import QtQuick.Window

Window {
    id: cover

    // The compositor sizes a lock surface; nothing here decides its geometry.
    color: CelestinaTheme.canvas
    visible: false

    // The absolute path the shell chose for this output, and the same identity
    // as a correctly escaped local URL. Both are set from C++, which owns the
    // conversion so a space or a `#` in a filename stays filename data. Empty
    // until — and unless — the shell says otherwise.
    property string wallpaperSource: ""
    property url wallpaperUrl: ""

    // A path this session cannot decode is the same as no path at all.
    readonly property bool showingWallpaper: cover.wallpaperSource.length > 0
                                             && backdrop.status === Image.Ready

    // How far the session is set back, and how far out of focus it goes.
    //
    // These live here rather than in the shared theme on purpose: they are this
    // one surface's composition, not a material anything else is built from.
    //
    // The recession travels *down to* true scale, never below it. Shrinking the
    // backdrop past the output was tried first and it does not read as depth —
    // it reads as a black margin around the picture, because that is exactly
    // what it is. So the session enters slightly overscanned and settles onto
    // its own true geometry: the same gesture of something moving away, with no
    // canvas ever exposed at the edges.
    //
    // Landing on 1 has a second, larger payoff. It is precisely the scale the
    // session's own wallpaper is at, so the resting locked state already
    // matches the frame the compositor will reveal — which is what lets
    // `LOCK-1-C` uncover without a jump.
    readonly property real enteringScale: 1.06
    readonly property int recededBlurMax: 64
    readonly property real recededBlurMultiplier: 2.4

    // Whether this surface has legally presented a frame.
    //
    // Nothing may animate before it has. `ext-session-lock-v1` makes committing
    // a buffer before acknowledging the first configure a protocol error, and
    // the compositor answers it by killing the client — which leaves the
    // session locked, blank, and with nothing to type into. The acknowledgement
    // is collected synchronously while the surface is built, but Qt Quick
    // renders on its own thread, so a running animation is enough to have that
    // thread commit a frame before the acknowledgement lands. It is a race, and
    // it presented as an intermittently black lock.
    //
    // A swapped frame is the one signal that cannot lie about this: if a frame
    // was presented, the configure was necessarily acknowledged first.
    property bool surfaceReady: false

    onFrameSwapped: {
        if (cover.surfaceReady)
            return;
        cover.surfaceReady = true;
        // A cover the shell never gave a wallpaper to has nothing to wait for.
        // One whose picture is somehow already decoded recedes at once.
        if (cover.wallpaperSource.length === 0)
            cover.revealOverlay();
        else
            cover.recede();
    }

    // The retreat, set from C++ the instant PAM answers yes and for no other
    // reason. It uncovers nothing: the release is the lock session's, on a
    // timer this surface cannot see, reach or delay. All this does is put the
    // backdrop back where the session's own wallpaper already is, so the
    // compositor reveals the session on a frame that already matches it.
    //
    // A refusal never sets this. The receded, blurred state is exactly what a
    // wrong passphrase leaves behind.
    property bool retreating: false

    onRetreatingChanged: {
        if (!cover.retreating)
            return;

        // Whatever was still arriving stops arriving; there is no state worth
        // finishing on a session that is about to be uncovered.
        recession.stop();
        overlayArrival.stop();

        if (CelestinaTheme.reducedMotion) {
            overlay.opacity = 0;
            stage.scale = 1;
            defocused.opacity = 0;
            wash.opacity = 0;
            return;
        }
        retreat.start();
    }

    // Two separate states, because they answer to two different things and one
    // of them must never be able to cancel the other.
    //
    // They were one flag, and a 4K photograph took longer to decode than the
    // guard timer below took to fire. The guard settled the surface, the
    // recession found itself already done, and the picture arrived flat: blurred
    // but never set back. Whether a person can type is not the same question as
    // whether there is a picture to push away, and answering both with one flag
    // let the safety net quietly eat the design.
    property bool receded: false
    property bool overlayShown: false

    // The clock and the prompt arrive. Idempotent, and deliberately independent
    // of the backdrop: this is what a person needs in order to unlock, so
    // nothing about a photograph may gate it.
    function revealOverlay() {
        if (!cover.surfaceReady || cover.overlayShown)
            return;
        cover.overlayShown = true;

        if (CelestinaTheme.reducedMotion) {
            overlay.opacity = 1;
            return;
        }
        overlayArrival.start();
    }

    // The session is set back and goes out of focus. Only ever with a decoded
    // picture to do it to — there is no depth to suggest on an empty canvas, and
    // an empty stage sliding away from the canvas edges would be a travel a
    // person can see and cannot explain.
    function recede() {
        if (!cover.surfaceReady || cover.receded || !cover.showingWallpaper)
            return;
        cover.receded = true;

        // Reduced motion keeps the depth and drops the travel: the same
        // composition, arrived at without the journey.
        if (CelestinaTheme.reducedMotion) {
            stage.scale = 1;
            defocused.opacity = 1;
            wash.opacity = 1;
            cover.revealOverlay();
            promptCard.refreshBackdrop();
            return;
        }
        recession.start();
    }

    // The prompt must appear. A slow disk, a format this build cannot decode or
    // a path that vanished between the shell reading it and this process
    // opening it are all reasons a photograph never arrives — and none of them
    // is a reason to leave a locked person with nothing to type into.
    //
    // It reveals the overlay and nothing else. A picture that arrives after this
    // has fired still recedes, just underneath a prompt that is already there.
    Timer {
        interval: CelestinaTheme.motionCeiling * 2
        // Only once the surface may legally move. Started any earlier, this
        // would be one more thing racing the first configure.
        running: cover.surfaceReady
        onTriggered: cover.revealOverlay()
    }

    // The ordinary path: the picture decoded, so the recession has something to
    // set back. The wallpaper usually arrives after this surface is already
    // mapped, which is why this is an event and not a startup step.
    onShowingWallpaperChanged: {
        if (cover.showingWallpaper)
            cover.recede();
    }

    // What the last attempt came back as, as a `LockAuthenticator.Verdict`.
    // Negative until something has been tried.
    property int lastVerdict: -1
    readonly property bool checking: lockAuthenticator.busy

    // The wording lives here, in QML, because the authenticator reports a
    // verdict and never a sentence. A refusal and an unavailable verifier are
    // deliberately different words: one means "that was not the passphrase",
    // the other means "this machine could not ask", and telling a person to
    // retype in the second case would be a lie.
    readonly property string message: {
        if (cover.checking)
            return qsTr("Comprobando…");
        if (cover.lastVerdict === LockAuthenticator.Refused)
            return qsTr("Contraseña incorrecta.");
        if (cover.lastVerdict === LockAuthenticator.Unavailable)
            return qsTr("No se pudo comprobar la contraseña.");
        return "";
    }

    Connections {
        target: lockAuthenticator

        function onAnswered(verdict) {
            cover.lastVerdict = verdict;
            // Cleared whatever the answer: a passphrase left on screen is one
            // somebody can read over a shoulder, and a correct one is about to
            // be irrelevant anyway.
            field.clear();
            if (verdict !== LockAuthenticator.Authenticated)
                field.forceActiveFocus();
        }
    }

    // The lock's language is Spanish by construction, exactly as the panel's
    // is, so the weekday and month are asked for in it rather than inherited
    // from whatever locale this process started with — a lock spawned from a
    // C-locale service otherwise renders "Friday 14 de August", which is what
    // it did before this line existed.
    readonly property var uiLocale: Qt.locale("es_ES")

    // The clock. It ticks to the minute, and only while this is on screen:
    // a locked machine should not be waking for a second hand nobody asked
    // for.
    QtObject {
        id: now

        property date value: new Date()
    }

    Timer {
        interval: 1000
        running: cover.visible
        repeat: true
        onTriggered: now.value = new Date()
    }

    // Everything that recedes, and nothing that does not. The scale lives on
    // this one item so the sharp image, its blurred copy and the wash over both
    // travel together as one surface being set back — rather than three layers
    // that happen to move at the same time.
    Item {
        id: stage

        anchors.fill: parent
        transformOrigin: Item.Center
        // Plain values, not bindings on `receded`: the animation below owns
        // these while it runs, and a binding would snap them to the end state
        // the moment the flag flipped — leaving a travel with nowhere to go.
        //
        // It starts overscanned and travels to 1. Nothing here ever goes below
        // 1, so the output is covered edge to edge at every instant.
        scale: cover.enteringScale

        Image {
            id: backdrop

            anchors.fill: parent
            visible: cover.showingWallpaper
            source: cover.wallpaperUrl
            fillMode: Image.PreserveAspectCrop
            // Off the GUI thread: a large photograph decoded inline would stall
            // the one surface a person is waiting to type into.
            asynchronous: true
            cache: false
            // Read at the screen's size rather than the photograph's, so a
            // 6000-pixel image does not cost its full decoded size on every
            // output.
            sourceSize.width: cover.width
            sourceSize.height: cover.height
        }

        // The out-of-focus copy, faded over the sharp one. Its source is a still
        // image, so the pyramid runs when the picture arrives and not once per
        // frame of the travel.
        //
        // It is loaded rather than merely hidden, and that is not tidiness. An
        // effect whose source image carries no texture — an output with no
        // wallpaper, or one whose file never decoded — stopped this scene
        // rendering at all: the surface never committed a frame, the compositor
        // never confirmed the lock, and the screen stayed the compositor's own
        // blank with no clock and nothing to type into. `visible: false` was not
        // enough, because the effect still ran. On this surface a renderer that
        // draws nothing is a person locked out of their session, so the effect
        // does not exist until there is a decoded picture for it to blur.
        Loader {
            id: defocused

            anchors.fill: parent
            active: cover.showingWallpaper
            opacity: 0

            sourceComponent: MultiEffect {
                source: backdrop
                blurEnabled: true
                blur: 1
                blurMax: cover.recededBlurMax
                blurMultiplier: cover.recededBlurMultiplier
                // The same slight desaturation the shell's glass applies, so an
                // out-of-focus photograph stops competing with the type over it.
                saturation: CelestinaTheme.glassSaturation
                autoPaddingEnabled: false
            }
        }

        // Legibility, not decoration. A heavy blur evens a photograph out but
        // does not darken it, and white type over a bright wallpaper is still
        // white type over a bright wallpaper. This arrives with the recession
        // rather than before it, so the sharp first frame is the session's own
        // colour and not a dimmed version of it.
        Rectangle {
            id: wash

            anchors.fill: parent
            visible: cover.showingWallpaper
            opacity: 0
            color: CelestinaTheme.scrim
        }
    }

    // The overlay does not recede. It arrives over a session that already has,
    // which is what makes the two read as different depths rather than one
    // picture being resized.
    Column {
        id: overlay

        anchors.centerIn: parent
        spacing: CelestinaTheme.space3xl
        width: Math.min(parent.width * 0.4, 420)
        opacity: 0

        Column {
            width: parent.width
            spacing: CelestinaTheme.spaceXs

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: now.value.toLocaleTimeString(cover.uiLocale, "HH:mm")
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.features: CelestinaTheme.fontFeaturesTabular
                font.pixelSize: CelestinaTheme.fontDisplay * 2
                font.weight: CelestinaTheme.weightDemiBold
            }

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: now.value.toLocaleDateString(cover.uiLocale,
                                                   "dddd d 'de' MMMM")
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontTitle
            }
        }

        // The prompt, on the shell's own dense material rather than floating
        // loose on the canvas — the same anatomy every card in this shell has.
        GlassSurface {
            id: promptCard

            width: parent.width
            implicitHeight: prompt.implicitHeight + CelestinaTheme.space2xl * 2
            height: implicitHeight
            cornerRadius: CelestinaTheme.radiusLg
            // Now that this surface paints its own backdrop, the card can
            // sample it for real. No compositor blur is involved or possible —
            // the region behind this card is inside this very scene, which is
            // exactly what `InSceneCapture` is for. An output with no wallpaper
            // has nothing to sample and falls back to the dense readable fill,
            // which is what the lock showed before any of this existed.
            backdropMode: GlassSurface.InSceneCapture
            backdropSource: cover.showingWallpaper ? stage : null
            captureEnabled: cover.showingWallpaper
            // The sample is taken once the recession has settled, not on every
            // frame of it: a live capture here would re-render the whole
            // backdrop through this card for the length of the travel.
            liveCapture: false
            // The same anatomy the polkit prompt uses: a nearly transparent
            // carrier over the blurred backdrop, with the material that reads
            // as a surface belonging to the field inside it rather than to a
            // dark plate laid over the picture. A `ContentSurface` here made
            // the card a slab; the veil lets the wallpaper stay visible
            // through the one thing sitting on top of it.
            materialRole: GlassSurface.ContextualVeil

            Column {
                id: prompt

                anchors.centerIn: parent
                width: parent.width - CelestinaTheme.space2xl * 2
                spacing: CelestinaTheme.spaceMd

                // The passphrase field, in the same translucent material the
                // polkit prompt uses for its own.
                //
                // The recipe is written out here rather than imported: the
                // shell's `BackdropTextField` and `BackdropInk` live in the
                // shell's QML module, which is compiled into the shell's
                // executable and cannot be reached from this one. Both sides
                // read the same `CelestinaStyle` tokens, so they stay the same
                // material — but a change to one has to be made in the other.
                CelestinaTextField {
                    id: field

                    objectName: "celestina-lock-passphrase"
                    width: parent.width
                    echoMode: TextInput.Password
                    // Nothing more is typed once the session has been unlocked
                    // and is only waiting for its cover to leave.
                    enabled: !cover.checking && !cover.retreating
                    placeholderText: qsTr("Contraseña")
                    color: CelestinaTheme.text
                    placeholderTextColor: CelestinaTheme.textMuted
                    selectionColor: CelestinaTheme.surfaceSelected
                    selectedTextColor: CelestinaTheme.text
                    onAccepted: {
                        if (text.length > 0)
                            lockAuthenticator.authenticate(text);
                    }

                    // Replaces the control's opaque scheme-bound plate with the
                    // low-density fixed-ink material of the carrier around it,
                    // so the field reads as part of the glass rather than as a
                    // solid box dropped onto it.
                    background: Rectangle {
                        radius: field.fieldRadius
                        color: field.visualFocus
                               ? CelestinaTheme.badgeAccentFill
                               : CelestinaTheme.glassHighlight
                        opacity: field.visualFocus
                                 ? CelestinaTheme.decorationOpacitySoft / 2
                                 : CelestinaTheme.decorationOpacitySoft / 4
                        border.width: field.visualFocus
                                      ? CelestinaTheme.borderFocus
                                      : CelestinaTheme.borderHairline
                        border.color: field.visualFocus
                                      ? CelestinaTheme.text
                                      : CelestinaTheme.divider

                        Behavior on color {
                            enabled: !CelestinaTheme.reducedMotion

                            ColorAnimation {
                                duration: CelestinaTheme.motionFast
                                easing.type: CelestinaTheme.easeStandard
                            }
                        }
                    }
                }

                Text {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    visible: cover.message.length > 0
                    text: cover.message
                    color: cover.lastVerdict === LockAuthenticator.Refused
                           ? CelestinaTheme.danger : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                    wrapMode: Text.WordWrap
                }
            }
        }
    }

    // The recession, and the overlay arriving a moment into it. The overlay is
    // deliberately late: starting both together reads as one cross-fade, while
    // letting the backdrop move first reads as something being set down behind
    // something else.
    //
    // Duration lives here rather than in a `Behavior` because the sample below
    // has to be taken when the travel ends, and a behaviour has no end to hang
    // that on.
    ParallelAnimation {
        id: recession

        NumberAnimation {
            target: stage
            property: "scale"
            to: 1
            duration: CelestinaTheme.motionSlow
            easing.type: CelestinaTheme.easeStandard
        }

        NumberAnimation {
            targets: [defocused, wash]
            property: "opacity"
            to: 1
            duration: CelestinaTheme.motionSlow
            easing.type: CelestinaTheme.easeStandard
        }

        // The overlay is asked for rather than animated here, so that a prompt
        // the guard already revealed is not faded in a second time.
        SequentialAnimation {
            PauseAnimation { duration: CelestinaTheme.motionFast }
            ScriptAction { script: cover.revealOverlay() }
        }

        // Only now is there a settled backdrop to sample. Taken once, because
        // nothing behind this card moves again while the session stays locked.
        onFinished: promptCard.refreshBackdrop()
    }

    // The recession, played backwards. The prompt leaves first, then the
    // session comes forward and back into focus — the reverse order of its
    // arrival, so the last thing a person sees is their own wallpaper exactly
    // as it will be a moment later without this surface in front of it.
    //
    // It lands on scale 1 and no blur because that is precisely the state the
    // session's own wallpaper is in. Uncovering onto anything else would be the
    // jump this whole checkpoint exists to remove.
    //
    // The scale is animated even though the resting state is already 1, because
    // somebody who types quickly can unlock while the entrance is still
    // travelling. This is what guarantees the backdrop arrives at true scale
    // from wherever it happened to be.
    ParallelAnimation {
        id: retreat

        NumberAnimation {
            target: overlay
            property: "opacity"
            to: 0
            duration: CelestinaTheme.motionNormal
            easing.type: CelestinaTheme.easeStandard
        }

        SequentialAnimation {
            PauseAnimation { duration: CelestinaTheme.motionFast }

            ParallelAnimation {
                NumberAnimation {
                    target: stage
                    property: "scale"
                    to: 1
                    duration: CelestinaTheme.motionSlow
                    easing.type: CelestinaTheme.easeStandard
                }

                NumberAnimation {
                    targets: [defocused, wash]
                    property: "opacity"
                    to: 0
                    duration: CelestinaTheme.motionSlow
                    easing.type: CelestinaTheme.easeStandard
                }
            }
        }
    }

    // The overlay's own arrival, separate from the recession because it also
    // happens on covers that have no backdrop to recede.
    NumberAnimation {
        id: overlayArrival

        target: overlay
        property: "opacity"
        to: 1
        duration: CelestinaTheme.motionNormal
        easing.type: CelestinaTheme.easeStandard
    }

    // Focus, and nothing else. Anything that moved the scene here would move it
    // before this surface has presented a legal frame — which is the race that
    // `surfaceReady` exists to close.
    Component.onCompleted: field.forceActiveFocus()
}
