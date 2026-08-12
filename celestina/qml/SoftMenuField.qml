// The visual field shared by every PANEL-1 interactive menu surface.
//
// One nearly transparent compositor-backed card owns the menu. Slightly denser
// neutral sections divide its content without publishing more blur regions or
// adding an exterior halo.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
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
    // PANEL-1-J. An attached surface is born as a drop at its own seam and
    // falls into place. Progress is the geometry's own input, not a transform
    // over a finished shape: every frame is a real droplet outline, and 1 is
    // exactly the settled geometry, so the motion cannot move where a surface
    // ends up. Reduced motion never leaves the settled value.
    readonly property bool fallsIntoPlace: root.edgeAttachmentRequested
                                           && !root.reducedMotion
    property real attachmentProgress: root.fallsIntoPlace ? 0 : 1
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
    readonly property real attachmentFadeIn: root.fallsIntoPlace
            ? Math.max(0, Math.min(1, root.attachmentProgress / 0.2))
            : 1
    property real retireOpacity: 1
    readonly property real attachmentContentOpacity: root.attachmentFadeIn
                                                     * root.retireOpacity

    // A dismissed surface is destroyed by its host, so the fade has to happen
    // before that: `SoftMenu` starts it on the popup's `aboutToHide`, which
    // runs while the exit transition still has the window alive.
    function retire() {
        if (root.reducedMotion) {
            root.retireOpacity = 0;
            return;
        }
        retireFade.start();
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
                  root.attachmentProgress)
            : root.edgeShapeActive
            ? EdgeAttachedGeometry.topAttachedMembrane(
                  root.edgePaneWidth, root.edgePaneHeight,
                  -root.edgePaneX, -root.edgePaneY,
                  root.width, root.height,
                  root.attachmentAnchorLeftAtBody - root.edgePaneX,
                  root.attachmentAnchorRect.width,
                  CelestinaTheme.radiusMd,
                  root.attachmentProgress)
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

    function reveal() {
        root.revealed = true;
        root.beginDropFall();
        root.scheduleGlassCollection();
    }

    // Idempotent: a route that reveals twice replays nothing, and a settled
    // surface never falls again. A surface that does not fall resolves to its
    // settled geometry instead of waiting for an animation that never runs.
    function beginDropFall() {
        if (!root.fallsIntoPlace) {
            root.attachmentProgress = 1;
            return;
        }
        if (dropFall.running || root.attachmentProgress >= 1)
            return;

        dropFall.start();
    }

    function collectGlass() {
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
                    const at = child.mapToItem(null, 0, 0);
                    const rect = Qt.rect(at.x, at.y, child.width, child.height);
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
            duration: CelestinaTheme.motionNormal
            easing.type: CelestinaTheme.easeStandard
        }

        NumberAnimation {
            target: root
            property: "attachmentProgress"
            to: 1
            duration: CelestinaTheme.motionFast
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

    Item {
        id: content
        objectName: "celestina-soft-menu-content"

        anchors.fill: parent
        transformOrigin: Item.Top
        // An edge-attached pane must never scale away from y=0. It keeps the
        // established opacity reveal, while floating fields retain the small
        // scale-up motion and reduced-motion still resolves immediately.
        scale: root.edgeAttachmentRequested
               || !root.animateReveal || root.revealed || root.reducedMotion
               ? 1 : 0.92
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
