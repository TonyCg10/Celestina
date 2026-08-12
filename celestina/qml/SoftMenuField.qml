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
    default property alias contentData: content.data

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
                  root.attachmentSideRight)
            : root.edgeShapeActive
            ? EdgeAttachedGeometry.topAttachedMembrane(
                  root.edgePaneWidth, root.edgePaneHeight,
                  -root.edgePaneX, -root.edgePaneY,
                  root.width, root.height,
                  root.attachmentAnchorLeftAtBody - root.edgePaneX,
                  root.attachmentAnchorRect.width,
                  CelestinaTheme.radiusMd)
            : ({"path": "", "edgePath": "", "polygon": [],
                "tension": 0, "waistWidth": 0, "waistCenter": 0})
    readonly property real attachmentTension: root.edgeSilhouette.tension
    readonly property real attachmentWaistWidth: root.edgeSilhouette.waistWidth
    // Convert the silhouette's pane-local result back into this field's local
    // coordinates. Contracts can then prove that the waist follows the icon
    // even when edgePaneX expands for a clamped or outlying anchor.
    readonly property real attachmentWaistCenterAtBody: root.edgeShapeActive
            ? root.edgePaneX + root.edgeSilhouette.waistCenter : 0

    function reveal() {
        root.revealed = true;
        root.scheduleGlassCollection();
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
        opacity: !root.animateReveal || root.revealed || root.reducedMotion ? 1 : 0

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
