// The visual field shared by every PANEL-1 interactive menu surface.
//
// One nearly transparent compositor-backed card owns the menu. Slightly denser
// neutral sections divide its content without publishing more blur regions or
// adding an exterior halo.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window
import "EdgeAttachedGeometry.js" as EdgeAttachedGeometry

Item {
    id: root
    objectName: "celestina-soft-menu-field"

    required property bool reducedMotion
    required property BackdropInk ink
    // The carried content is a sibling layer above the glass rather than a
    // peer of it, so a falling drop can reveal what it carries without
    // fading the drop itself.
    default property alias contentData: body.data

    property bool animateReveal: true
    property bool revealed: false
    property bool compositorBlurAvailable: false
    // Only a surface opened by a real panel control grows into the top edge.
    // Command/keybind routes leave this false and keep the rounded floating
    // field. surfacePosition and openerRect use output-local coordinates.
    property bool attachedToTop: false
    property point surfacePosition: Qt.point(root.x, root.y)
    property rect openerRect: Qt.rect(0, 0, 0, 0)
    // The control remains the placement/focus opener. This second rectangle
    // is the exact icon inside it and positions only the transparent
    // membrane's narrow waist; neither its capsule nor menu contents join it.
    property rect attachmentAnchorRect: Qt.rect(0, 0, 0, 0)
    // Output-local lower edge of the continuous panel veil. The host supplies
    // it from the real panel surface; no menu hard-codes the bar height.
    property real attachmentStartY: -1
    // A child menu born from a row of another menu attaches sideways instead:
    // the seam is this card's own vertical edge facing its parent surface,
    // which the host places flush against the parent card. The droplet then
    // grows out of that edge at the invoking row's icon height. The gap is
    // the horizontal travel the membrane crosses inside this same window.
    property bool attachedToSide: false
    property bool attachmentSideRight: false
    property real sideAttachmentGap: 0
    property Item glassRoot: content
    property var glassRects: []
    property var glassRegions: []

    readonly property bool topAttachmentRequested:
            root.attachedToTop
            && root.openerRect.width > 0
            && root.openerRect.height > 0
            && root.attachmentAnchorRect.width > 0
            && root.attachmentAnchorRect.height > 0
            && root.attachmentStartY >= 0
    readonly property bool sideAttachmentRequested:
            root.attachedToSide
            && root.attachmentAnchorRect.width > 0
            && root.attachmentAnchorRect.height > 0
            && root.sideAttachmentGap > 0
    readonly property bool edgeAttachmentRequested:
            root.topAttachmentRequested || root.sideAttachmentRequested
    readonly property bool edgeShapeActive:
            root.sideAttachmentRequested
            || (root.topAttachmentRequested
                && root.surfacePosition.y > root.attachmentStartY)
    // PANEL-1-J gave the drop its morphing fall out of the seam; the author
    // then asked for the whole card instead (2026-08-13): everything that
    // hangs from the bar falls rigid from beyond the screen's top edge to its
    // resting place, with the same short elastic recoil. The side push — the
    // tray child — keeps the morph. So the top route draws its settled
    // membrane from the first frame and `attachmentProgress` drives a pure
    // translation of the whole assembly; the side route still feeds the
    // geometry. Reduced motion never leaves the settled value on either.
    readonly property bool fallsIntoPlace: root.edgeAttachmentRequested
                                           && !root.reducedMotion
    property real attachmentProgress: root.fallsIntoPlace ? 0 : 1
    readonly property real membraneProgress: root.sideAttachmentRequested
                                             ? root.attachmentProgress : 1
    // Where the descending card's top sits below the seam, per frame: the
    // geometry's own travel input. Negative while the card is still leaving
    // the bar — no gap yet, so no membrane yet; the drop grows and stretches
    // in the gap the descent opens, which is the forming the author asked
    // for, instead of a finished drop riding down as a block.
    readonly property real entryBodyY: -root.edgePaneY + root.entryOffsetY
    // How far the assembly starts above its resting place: its own bottom at
    // the screen's top edge, so no part of it exists on screen at progress 0.
    readonly property real entryTravel: root.surfacePosition.y + root.height
    // The recoil is a fixed short dip, not a fraction of the travel: five
    // percent of a tall menu's whole flight would be a bounce, not a landing.
    // Deep enough to be seen landing, and recovered over a real beat below.
    readonly property real entryBounceDepth: CelestinaTheme.spaceLg
    // Between constant time and constant speed: one duration over a tall
    // menu's flight was a blur, but paying the full per-pixel price made the
    // same menu a slow curtain. The base beat covers the common card and each
    // extra pixel of flight adds only a fraction, capped at the shell's slow
    // beat — a tall menu reads a little heavier, never sluggish.
    readonly property int entryDuration: Math.min(
            CelestinaTheme.motionSlow,
            CelestinaTheme.motionNormal
            + Math.round(Math.max(0, root.entryTravel - 300) * 0.25))
    readonly property real entryOffsetY:
            root.topAttachmentRequested && root.fallsIntoPlace
            ? (root.attachmentProgress <= 1
               ? (root.attachmentProgress - 1) * root.entryTravel
               : (root.attachmentProgress - 1) / 0.05 * root.entryBounceDepth)
            : 0
    // What the drop is carrying rides inside it. The body rectangle comes
    // from the same geometry as the outline, so the content is bounded by the
    // exact drop that holds it and travels with it — including through the
    // recoil — instead of waiting at the resting place for the glass to
    // arrive. Its own layout never changes: it is translated and clipped, so
    // no row is ever stretched or reflowed by the motion.
    readonly property rect attachmentBodyRect: root.edgeShapeActive
            ? Qt.rect(root.edgePaneX + root.edgeSilhouette.openRect.x,
                      root.edgePaneY + root.edgeSilhouette.openRect.y,
                      root.edgeSilhouette.openRect.width,
                      root.edgeSilhouette.openRect.height)
            : Qt.rect(0, 0, root.width, root.height)
    // Only while the drop is still moving: a settled body is exactly its own
    // card, so clipping it would cost a render pass and buy nothing.
    readonly property bool attachmentClipsContent: root.fallsIntoPlace
            && root.sideAttachmentRequested
            && (dropFall.running || root.attachmentProgress < 1)
    // The content rides the drop on every route — carried by the body window
    // on overlays, translated with the popup for real menus — so the fade is
    // not a reveal. It is a short one at each end: in across the first fifth
    // of the fall so the card does not pop into existence at the bar, and out
    // when the surface is dismissed so it does not vanish mid-air. A slow
    // fade-in here read, in the author's recording, as content appearing at
    // the destination instead of falling with its card.
    //
    // The glass and everything it carries share this one value, so the popup
    // that a real Menu keeps outside the field fades in step with it.
    // Only the side morph fades in: a card sliding in from beyond the screen
    // edge is never seen popping into existence, so it arrives at full
    // opacity and simply enters.
    readonly property real attachmentFadeIn:
            root.fallsIntoPlace && root.sideAttachmentRequested
            ? Math.max(0, Math.min(1, root.attachmentProgress / 0.2))
            : 1
    property real retireOpacity: 1
    // The universal departure the author asked for: every surface leaves by
    // shrinking into the screen while it fades — one block, glass and content
    // together, because both live under `content` and this scales that.
    property real retireScale: 1
    readonly property real attachmentContentOpacity: root.attachmentFadeIn
                                                     * root.retireOpacity

    // A dismissed surface is destroyed by its host, so the fade has to happen
    // before that: `SoftMenu` starts it on the popup's `aboutToHide`, which
    // runs while the exit transition still has the window alive.
    function retire() {
        if (root.reducedMotion) {
            root.retireOpacity = 0;
            root.retireScale = 1;
            return;
        }
        retireFade.start();
        retireShrink.start();
    }
    // The panel lives below the overlay layer. Start this window's material at
    // the continuous bar backdrop's lower edge. The overlay supplies only the
    // exposed connector and body; it never repaints or reblurs the bar or the
    // icon or its unchanged content capsule above that boundary.
    // Y of the seam where this veil continues from the panel. Only the
    // droplet's narrow mouth touches it, centred on the icon below.
    readonly property real attachmentSeamOnSurface:
            root.attachmentStartY
    readonly property real attachmentAnchorLeftAtBody:
            root.attachmentAnchorRect.x - root.surfacePosition.x
    readonly property real attachmentAnchorRightAtBody:
            root.attachmentAnchorLeftAtBody + root.attachmentAnchorRect.width
    readonly property real edgePaneX: root.sideAttachmentRequested
            ? (root.attachmentSideRight ? 0 : -root.sideAttachmentGap)
            : root.edgeShapeActive
              ? Math.min(0, root.attachmentAnchorLeftAtBody)
              : 0
    readonly property real edgePaneRight: root.sideAttachmentRequested
            ? (root.attachmentSideRight
               ? root.width + root.sideAttachmentGap : root.width)
            : root.edgeShapeActive
              ? Math.max(root.width, root.attachmentAnchorRightAtBody)
              : root.width
    readonly property real edgePaneY: root.edgeShapeActive
                                      && !root.sideAttachmentRequested
            ? root.attachmentSeamOnSurface - root.surfacePosition.y : 0
    readonly property real edgePaneWidth: root.edgePaneRight - root.edgePaneX
    readonly property real edgePaneHeight: root.height - root.edgePaneY
    readonly property var edgeSilhouette: root.sideAttachmentRequested
            ? EdgeAttachedGeometry.sideAttachedMembrane(
                  root.edgePaneWidth, root.edgePaneHeight,
                  -root.edgePaneX, 0,
                  root.width, root.height,
                  root.attachmentAnchorRect.y - root.surfacePosition.y,
                  root.attachmentAnchorRect.height,
                  CelestinaTheme.radiusMd,
                  root.attachmentSideRight,
                  root.membraneProgress)
            : root.edgeShapeActive
            ? (root.entryBodyY > 0.5
               ? EdgeAttachedGeometry.topAttachedMembrane(
                     root.edgePaneWidth, root.edgePaneHeight,
                     -root.edgePaneX, root.entryBodyY,
                     root.width, root.height,
                     root.attachmentAnchorLeftAtBody - root.edgePaneX,
                     root.attachmentAnchorRect.width,
                     CelestinaTheme.radiusMd,
                     root.membraneProgress)
               : EdgeAttachedGeometry.emergingBodyPath(
                     -root.edgePaneX, root.entryBodyY,
                     root.width, root.height,
                     CelestinaTheme.radiusMd))
            : ({"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistCenter": 0,
                "openRect": {"x": 0, "y": 0, "width": 0, "height": 0}})
    readonly property real attachmentTension: root.edgeSilhouette.tension
    readonly property real attachmentWaistWidth: root.edgeSilhouette.waistWidth
    // Convert the silhouette's pane-local result back into this field's local
    // coordinates. Contracts can then prove that the waist follows the icon
    // even when edgePaneX expands for a clamped or outlying anchor.
    readonly property real attachmentWaistCenterAtBody: root.edgeShapeActive
            ? root.edgePaneX + root.edgeSilhouette.waistCenter : 0

    // A reveal request waits for the window's next presented frame. Every
    // route that revealed at creation had its first commit shown with the
    // scene still mid-layout — at 4K the OSD's finished card covered the
    // bar's own icons for exactly one frame before its placement applied,
    // and the menus' falls began from geometry nobody had settled. The
    // timer is the offscreen fallback, where a window nobody presents may
    // never swap a frame at all.
    property bool revealQueued: false
    function reveal() {
        if (root.revealed || root.revealQueued)
            return;
        root.revealQueued = true;
        revealSwap.target = root.Window.window;
        revealSwapFallback.start();
    }
    function revealNow() {
        revealSwap.target = null;
        revealSwapFallback.stop();
        root.revealQueued = false;
        root.revealed = true;
        root.beginDropFall();
        root.scheduleGlassCollection();
    }
    Connections {
        id: revealSwap

        target: null
        function onFrameSwapped() {
            root.revealNow();
        }
    }
    Timer {
        id: revealSwapFallback

        interval: 50
        onTriggered: root.revealNow()
    }

    // One fall per surface, ever. `hasFallen` rather than the progress value,
    // because the two disagree in exactly one case: a child menu is revealed
    // before its host has turned the side attachment on, which forces the
    // progress to 1 with no fall having happened — and when the flag then
    // arrives, the sideways push the author asked for must still run once.
    property bool hasFallen: false

    // Idempotent: a route that reveals twice replays nothing, and a surface
    // that has fallen never falls again. A surface that does not fall resolves
    // to its settled geometry instead of waiting for an animation that never
    // runs.
    function beginDropFall() {
        if (!root.fallsIntoPlace) {
            root.attachmentProgress = 1;
            return;
        }
        root.startFall();
    }

    // Whether this surface has been shown to anyone: flipped by the first
    // frame the compositor actually presented. A fresh card-sized layer
    // surface takes the compositor a configure round-trip to map, and a fall
    // started at creation plays out entirely inside that gap — the author's
    // recording shows the settled card materialising in one frame, the whole
    // push already spent unseen. The fall therefore holds at its first frame
    // until there is a first frame to hold on.
    property bool surfacePresented: false
    property bool fallQueued: false

    // The wait above has a race a card created inside its window loses: at
    // `Component.onCompleted` the `Window.window` attachment can still be
    // null, and by the time the Connections below retargets, the mapped
    // window's first — and, for a static scene, only — frames have already
    // swapped. Nothing else ever renders, the signal never comes, and the
    // queued fall holds the content at opacity zero forever. So queuing a
    // fall asks the window for one more frame, and so does the window
    // attachment resolving, whichever happens last.
    // `var`, not `Window`: the test harness hosts fields in a QQuickView,
    // which is a window but not the QML Window type.
    readonly property var hostWindow: root.Window.window
    onHostWindowChanged: root.nudgePresentation()

    function nudgePresentation() {
        if (!root.surfacePresented || root.fallQueued)
            presentationFallback.restart();
        if (!root.surfacePresented && root.fallQueued && root.hostWindow)
            root.hostWindow.requestUpdate();
    }

    // The last resort under the nudge: measured on the nested session, a
    // quiet surface's window can present without its `frameSwapped` ever
    // reaching this field, and a fall that waits forever is a surface that
    // stays at opacity zero while its blur region announces where it should
    // have been. If no frame has been seen shortly after queuing, the fall
    // runs anyway: at worst its first frames play while the compositor is
    // still mapping, which is the small cost the wait existed to avoid — and
    // strictly better than never being seen at all.
    Timer {
        id: presentationFallback

        interval: CelestinaTheme.motionNormal
        onTriggered: {
            if (!root.fallQueued)
                return;
            root.fallQueued = false;
            root.surfacePresented = true;
            dropFall.start();
        }
    }

    Connections {
        target: root.hostWindow

        function onFrameSwapped() {
            if (root.surfacePresented)
                return;
            root.surfacePresented = true;
            if (root.fallQueued) {
                root.fallQueued = false;
                dropFall.start();
            }
        }
    }

    function startFall() {
        if (dropFall.running || root.hasFallen)
            return;

        root.hasFallen = true;
        root.attachmentProgress = 0;
        if (!root.surfacePresented) {
            root.fallQueued = true;
            root.nudgePresentation();
            return;
        }
        dropFall.start();
    }

    // The attachment can arrive after the reveal: the host sets the side flag
    // on an already-created window, synchronously but later in the same call.
    // A revealed surface that never fell starts its push here.
    //
    // Everything read below is a plain input property, never a derived
    // binding. Inside the change dispatch of one of its own inputs, a lazy
    // derived binding can still answer with its previous value — measured:
    // with the side flag, the gap and the anchor all set, the derived request
    // read false from in here and true from the very next statement outside —
    // so this recomputes the request from the raw inputs itself.
    function beginLateFall() {
        if (!root.revealed || root.reducedMotion)
            return;
        const anchorReal = root.attachmentAnchorRect.width > 0
                           && root.attachmentAnchorRect.height > 0;
        const sideReady = root.attachedToSide && anchorReal
                          && root.sideAttachmentGap > 0;
        const topReady = root.attachedToTop && anchorReal
                         && root.openerRect.width > 0
                         && root.openerRect.height > 0
                         && root.attachmentStartY >= 0;
        if (sideReady || topReady)
            root.startFall();
    }
    onAttachedToSideChanged: root.beginLateFall()
    onSideAttachmentGapChanged: root.beginLateFall()
    onAttachedToTopChanged: root.beginLateFall()
    onAttachmentAnchorRectChanged: root.beginLateFall()

    function collectGlass() {
        // Not before the reveal. The compositor's material cannot fade — a
        // region exists or it does not — so a region armed while the paint is
        // still at zero is a bare milky slab on the wallpaper, which the
        // author recorded leading the card by several frames on every open.
        // Collected only once the reveal is running, the snap lands under
        // paint that is already forming, and the two read as one block.
        if (!root.revealed) {
            root.glassRects = [];
            root.glassRegions = [];
            return;
        }
        const foundRects = [];
        const foundRegions = [];
        const walk = function(item) {
            if (!item || item.children === undefined)
                return;

            for (let index = 0; index < item.children.length; ++index) {
                const child = item.children[index];
                if (child.objectName === "celestina-compositor-glass-region"
                    && child.visible
                    && child.width > 0 && child.height > 0) {
                    const rect = EdgeAttachedGeometry.mapRect(child);
                    foundRects.push(rect);
                    foundRegions.push({
                        "rect": rect,
                        "radius": child.radius,
                        "polygon": EdgeAttachedGeometry.mapPolygon(
                            child, child.polygon)
                    });
                }
                walk(child);
            }
        };
        walk(root.glassRoot);
        root.glassRects = foundRects;
        root.glassRegions = foundRegions;
    }

    function scheduleGlassCollection() {
        glassSettle.restart();
    }

    // The compositor region is expressed in window coordinates, not merely in
    // this field's local geometry. A layer configure or placement update may
    // translate the complete field while its immediate children stay at zero;
    // republish after either representation of that effective origin moves.
    onXChanged: root.scheduleGlassCollection()
    onYChanged: root.scheduleGlassCollection()
    onSurfacePositionChanged: root.scheduleGlassCollection()

    // The compositor region follows the falling drop frame by frame. The
    // settle debounce below exists for reveals whose geometry is briefly
    // wrong mid-animation; during the fall every frame's geometry is exact —
    // the outline and its sampled polygon come from the same function — and
    // debouncing it swallowed every frame and delivered only the landed
    // shape, so the whole opening played over an unblurred backdrop and the
    // blur arrived as a pop at the end. ext-background-effect is
    // double-buffered and the blur controller re-arms only when the region
    // really changed, so publishing per frame costs one region update on a
    // frame that is being committed anyway.
    // Per frame on every falling route, exactly as PANEL-1-P settled for the
    // morph: the region and the outline come from the same function, the
    // effect state is double-buffered, and a card without its blur for the
    // length of an animation is a card that visibly loses its glass. The
    // emergence polygon is clamped at the seam, so no frame ever asks the
    // compositor to blur the bar's own rows.
    onAttachmentProgressChanged: {
        if (root.edgeShapeActive)
            root.collectGlass();
    }
    onEntryOffsetYChanged: {
        // Deferred out of this dispatch on purpose: inside it, the region's
        // own lazy polygon binding can still answer with the previous frame's
        // shape — the emergence phase publishes none — which is the exact
        // stale-read this file already documents for the side gap. The
        // collector re-checks the offset itself, so a deferral that lands
        // mid-flight publishes nothing.
        if (root.entryOffsetY === 0)
            Qt.callLater(root.collectGlass);
    }

    // The fall is two tokened parts: it decelerates from its very first frame
    // — the way every other motion in this shell moves — gliding a little
    // past its resting place as the membrane takes its weight, and is then
    // drawn back up to rest over a short recovery. It spanned the complete
    // `motionCeiling` until the author asked for a little more pace; the
    // recovery is what gave that back, because it is the part with the least
    // distance to cover.
    //
    // The card now falls whole from the bar, so the full range is the right
    // one to travel: at 0 the complete, full-sized body hangs at the seam and
    // the connector's own 20..36-pixel travel is the entire distance — the
    // motion is inherently small. The morphing variants that needed a partial
    // range to stay subtle (growing out of the mouth, the affine ride) were
    // all rejected by the author; so were a flat monotone curve, an
    // `easeEmphasized` spring snap, and an accelerating `easeExit` opening
    // that read as mechanical.
    SequentialAnimation {
        id: dropFall
        objectName: "celestina-attachment-drop-fall"

        NumberAnimation {
            target: root
            property: "attachmentProgress"
            from: 0
            to: 1.05
            duration: root.topAttachmentRequested ? root.entryDuration
                                                  : CelestinaTheme.motionNormal
            easing.type: CelestinaTheme.easeStandard
        }

        NumberAnimation {
            target: root
            property: "attachmentProgress"
            to: 1
            // The landing recoil is meant to be read, not merely to exist:
            // a hundred milliseconds disappeared under the fall before it.
            duration: root.topAttachmentRequested ? CelestinaTheme.motionNormal
                                                  : CelestinaTheme.motionFast
            easing.type: CelestinaTheme.easeStandard
        }
    }

    NumberAnimation {
        id: retireFade
        objectName: "celestina-attachment-retire-fade"

        target: root
        property: "retireOpacity"
        to: 0
        duration: CelestinaTheme.motionFast
        easing.type: CelestinaTheme.easeExit
    }

    NumberAnimation {
        id: retireShrink

        target: root
        property: "retireScale"
        to: 0.92
        duration: CelestinaTheme.motionFast
        easing.type: CelestinaTheme.easeExit
    }

    Timer {
        id: glassSettle

        // A floating rectangle must be sampled after scale-up reaches 1.0;
        // publishing its transformed origin with its untransformed size midway
        // through that animation creates a blur region larger than the card.
        // The attached route never scales, so it uses the short settle and
        // arms its polygon with the opacity reveal instead of visibly later.
        interval: root.animateReveal && !root.reducedMotion
                  && !root.edgeAttachmentRequested
                  ? CelestinaTheme.motionNormal + CelestinaTheme.space3xl
                  : 80
        repeat: false
        onTriggered: root.collectGlass()
    }

    // The author asked for the fall to happen behind the bar, and a layer
    // cannot slide under the panel's own surface: the overlay layer is above
    // it by protocol. So the field simply never paints above the seam while
    // it is entering — the assembly emerges from under the bar instead of
    // flying over it. The window spans the pane's own extents so nothing is
    // cut sideways, and the clip costs nothing at rest.
    Item {
        id: entryWindow

        x: Math.min(0, root.edgePaneX)
        // On the silhouette alone this held only while `edgeShapeActive` was
        // already true; the first frames of a fall can run before the
        // silhouette is built, and in the author's 4K recording the body
        // painted from the screen's top edge over the bar's own clock in
        // exactly those frames. A top attachment now clips at the seam for
        // the whole entry, silhouette or not.
        y: root.topAttachmentRequested
           ? root.attachmentStartY - root.surfacePosition.y : 0
        width: Math.max(root.width, root.edgePaneRight) - entryWindow.x
        height: root.height - entryWindow.y
        clip: root.entryOffsetY !== 0
              || (root.topAttachmentRequested && root.fallsIntoPlace
                  && root.attachmentProgress < 1)

    Item {
        id: content
        objectName: "celestina-soft-menu-content"

        x: -entryWindow.x
        y: -entryWindow.y
        width: root.width
        height: root.height
        // Centre-origin, so both the small entry scale and the departing
        // shrink read as depth — toward and away from the person — rather
        // than as hanging growth from the top edge.
        transformOrigin: Item.Center

        // An edge-attached pane must never scale away from y=0. It keeps the
        // established opacity reveal, while floating fields retain the small
        // scale-up motion and reduced-motion still resolves immediately.
        scale: (root.edgeAttachmentRequested
                || !root.animateReveal || root.revealed || root.reducedMotion
                ? 1 : 0.92) * root.retireScale
        // The whole surface, glass included, carries the fade at both ends.
        opacity: (!root.animateReveal || root.revealed || root.reducedMotion
                  ? 1 : 0) * root.attachmentContentOpacity

        CompositorGlassRegion {
            x: root.edgePaneX
            y: root.edgePaneY
            width: root.edgePaneWidth
            height: root.edgePaneHeight
            z: -3
            blurAvailable: root.compositorBlurAvailable
            // A missing compositor sample still needs a contrast floor. The
            // live blur path below deliberately does not reuse this dark
            // fallback tint.
            fallbackColor: CelestinaTheme.glassTint
            radius: CelestinaTheme.radiusMd
            silhouettePath: root.edgeSilhouette.path
            polygon: root.edgeSilhouette.polygon
            onBlurRegionChanged: root.scheduleGlassCollection()
        }

        // The compositor or the region's fallback supplies the external
        // backdrop. Style remains the one material authority for this very
        // light veil and for every denser section above it.
        GlassSurface {
            x: root.edgePaneX
            y: root.edgePaneY
            width: root.edgePaneWidth
            height: root.edgePaneHeight
            z: -2
            objectName: "celestina-menu-body-tint"
            backdropMode: GlassSurface.ExternalBackdrop
            externalBackdropReady: true
            captureEnabled: false
            materialRole: GlassSurface.ContextualVeil
            materialTint: root.ink.materialTint
            cornerRadius: CelestinaTheme.radiusMd
            silhouettePath: root.edgeSilhouette.path
            silhouetteEdgePath: root.edgeSilhouette.edgePath
            elevation: 0
        }

        // Everything the surface carries, riding inside the drop. The window
        // is the momentary body and the content keeps its settled layout at
        // that window's origin, so rows emerge from the seam with the glass
        // instead of appearing at the resting place once it arrives. An
        // inside press is still stopped here while it is falling, so the
        // motion never leaks a click through to whatever an overlay uses to
        // dismiss itself.
        Item {
            id: bodyWindow
            objectName: "celestina-soft-menu-body-window"

            x: root.attachmentBodyRect.x
            y: root.attachmentBodyRect.y
            width: root.attachmentBodyRect.width
            height: root.attachmentBodyRect.height
            clip: root.attachmentClipsContent

            Item {
                id: body
                objectName: "celestina-soft-menu-body"

                width: root.width
                height: root.height
            }
        }

        Behavior on scale {
            // The complete attachment request is stable from construction,
            // unlike cardY while layer-shell configures the output-sized
            // surface. Never let bootstrap geometry briefly scale a real
            // monitor-edge mouth away from y=0; compatibility routes without
            // an attachment edge retain the floating animation.
            enabled: !root.reducedMotion && !root.edgeAttachmentRequested

            NumberAnimation {
                duration: CelestinaTheme.motionNormal
                easing.type: CelestinaTheme.easeEmphasized
                easing.overshoot: CelestinaTheme.overshoot
            }
        }

        Behavior on opacity {
            enabled: !root.reducedMotion

            NumberAnimation {
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }
    }
    }
}
