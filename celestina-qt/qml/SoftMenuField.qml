// SIMPLE-2. The contextual surface core — see DRAWING.md for the contract.
//
// A field is the block's background (scrim and shadow) with its glass
// cards on it, one fade, one region beat, one heartbeat. That is the whole
// file. Its ancestor was 800 lines of
// membrane silhouettes, drop falls and entry clips from the pre-reset
// system, kept inert for weeks; every property a host still reads survives
// below as plain data, and everything else is gone.
//
// Lifecycle, unchanged from SIMPLE-1 and driven by the hosts:
//   reveal()          queue the fade-in for the next presented frame
//   revealNow()       consume the presentation gate immediately (C++)
//   retire()          the one exit fade; irreversible per open
//   resetForReuse()   a persistent quiet carrier taking a fresh block
//   reviveForReuse()  a parked carrier resuming (C++, then a route re-reveal)
//   restPublishedGlass()  park: withdraw the published regions (C++)
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
    default property alias contentData: body.data

    // ── The background ────────────────────────────────────────────────────
    // A menu has NO containing panel (the author rejected one on sight).
    // What macOS really paints behind an open surface — measured off the
    // author's open/closed screenshots — is a FULL-SCREEN ~20 % scrim, bar
    // included, plus almost-nothing per-card shadows. The scrim spans this
    // whole carrier (which begins at the output's top edge) and rides the
    // same fade as the content. Quiet surfaces (toasts, the display) turn
    // it off: a notification must not darken the person's work.
    property bool castsShadow: true
    property bool dimsBackdrop: true
    property real panelRadius: CelestinaTheme.radiusLg

    // ── Placement inputs ──────────────────────────────────────────────────
    // Written by the hosts and read by the placement math and the tests.
    // Painting no longer derives anything from them; they are data.
    property bool attachedToTop: false
    property bool attachedToSide: false
    property bool attachmentSideRight: false
    property real sideAttachmentGap: 0
    property point surfacePosition: Qt.point(root.x, root.y)
    property rect openerRect: Qt.rect(0, 0, 0, 0)
    property rect attachmentAnchorRect: Qt.rect(0, 0, 0, 0)
    property real attachmentStartY: -1
    property bool compositorBlurAvailable: false
    property bool animateReveal: true
    property Item glassRoot: content

    // An anchored route whose icon rectangle has not arrived yet: the field
    // holds paint and publication until the lease lands, so no floating
    // fallback flashes detached from the bar.
    readonly property bool attachmentPending:
            root.attachedToTop
            && (root.attachmentAnchorRect.width <= 0
                || root.attachmentAnchorRect.height <= 0)
    readonly property bool topAttachmentRequested:
            root.attachedToTop
            && root.openerRect.width > 0 && root.openerRect.height > 0
            && root.attachmentAnchorRect.width > 0
            && root.attachmentAnchorRect.height > 0
            && root.attachmentStartY >= 0
    readonly property bool sideAttachmentRequested:
            root.attachedToSide
            && root.attachmentAnchorRect.width > 0
            && root.attachmentAnchorRect.height > 0
            && root.sideAttachmentGap > 0
    readonly property bool edgeAttachmentRequested:
            (root.attachedToTop
             && root.openerRect.width > 0 && root.openerRect.height > 0
             && root.attachmentAnchorRect.width > 0
             && root.attachmentAnchorRect.height > 0
             && root.attachmentStartY >= 0)
            || (root.attachedToSide
                && root.attachmentAnchorRect.width > 0
                && root.attachmentAnchorRect.height > 0
                && root.sideAttachmentGap > 0)

    // Kept as inert API: SoftMenu mirrors these while riding the field, and
    // the harnesses read them. Nothing moves them any more.
    readonly property rect attachmentBodyRect:
            Qt.rect(0, 0, root.width, root.height)
    readonly property bool attachmentClipsContent: false
    readonly property real entryOffsetY: 0
    readonly property real attachmentProgress: 1
    readonly property bool fallsIntoPlace: false
    readonly property bool edgeShapeActive: false

    // ── Lifecycle ─────────────────────────────────────────────────────────
    property bool revealed: false
    property bool revealQueued: false
    property bool retiring: false
    property real retireOpacity: 1
    // The universal departure lost its shrink with SIMPLE-1; the property
    // stays because SoftMenu mirrors it onto its popup.
    readonly property real retireScale: 1

    readonly property real presentationOpacity:
            root.revealed && !root.attachmentPending ? root.retireOpacity : 0

    function reveal() {
        if (root.revealed || root.revealQueued || root.retiring)
            return;
        root.revealQueued = true;
        revealSwap.target = root.Window.window;
        revealSwapFallback.start();
    }

    function revealNow() {
        revealSwap.target = null;
        revealSwapFallback.stop();
        root.revealQueued = false;
        if (root.retiring)
            return;
        root.revealed = true;
        root.scheduleGlassCollection();
    }

    function retire() {
        if (root.retiring)
            return;
        root.retiring = true;
        root.revealQueued = false;
        revealSwap.target = null;
        revealSwapFallback.stop();
        // The material comes down with the paint: an armed region under a
        // fading card is the milky slab this shell has recorded too often.
        root.glassRects = [];
        root.glassRegions = [];
        if (root.reducedMotion) {
            root.retireOpacity = 0;
            return;
        }
        retireFade.start();
    }

    function resetForReuse() {
        if (root.retiring)
            return;
        revealSwap.target = null;
        revealSwapFallback.stop();
        root.revealQueued = false;
        root.revealed = false;
        root.publishedGlassFingerprint = "";
        root.glassRects = [];
        root.glassRegions = [];
    }

    function reviveForReuse() {
        retireFade.stop();
        root.retiring = false;
        root.retireOpacity = 1;
        root.resetForReuse();
    }

    function restPublishedGlass() {
        root.publishedGlassFingerprint = "";
        root.glassRects = [];
        root.glassRegions = [];
    }

    // Kept for the harnesses that flip it by hand; production consumes the
    // window's frame swap through `reveal()`.
    property bool surfacePresented: false

    Connections {
        id: revealSwap

        target: null
        function onFrameSwapped() {
            root.surfacePresented = true;
            root.revealNow();
        }
    }
    Timer {
        id: revealSwapFallback

        interval: 50
        onTriggered: root.revealNow()
    }

    NumberAnimation {
        id: retireFade

        target: root
        property: "retireOpacity"
        to: 0
        duration: CelestinaTheme.motionExit
        easing.type: CelestinaTheme.easeStandard
    }

    // ── The pulse ─────────────────────────────────────────────────────────
    // A Wayland window that commits nothing stops receiving frame callbacks
    // on this compositor and everything aboard freezes; the beat keeps one
    // dirty commit in flight for the whole mapped life, and re-walks the
    // regions so layout drift can never strand the material (the bar's
    // capsules taught that lesson).
    Rectangle {
        id: fieldHeartbeat

        width: 1
        height: 1
        color: CelestinaTheme.clear

        Timer {
            interval: 500
            repeat: true
            running: root.visible && root.Window.window !== null
                     && !root.retiring
            onTriggered: {
                fieldHeartbeat.x = fieldHeartbeat.x === 0 ? 1 : 0;
                if (root.Window.window)
                    root.Window.window.requestUpdate();
                root.collectGlass();
            }
        }
    }

    // ── The region beat ───────────────────────────────────────────────────
    // One walk finds every glass card's marker under the content — one per
    // MenuSection, and on the toast route one per notification. Published only at
    // rest (the fade fully landed) with a hand-built fingerprint: a QML rect
    // stringifies as "{}", so JSON of the list was one constant and the beat
    // ran mute once already.
    property var glassRects: []
    property var glassRegions: []
    property string publishedGlassFingerprint: ""

    function collectGlass() {
        if (root.retiring)
            return;
        if (!root.revealed || root.attachmentPending || !root.visible
                || root.opacity <= 0 || content.opacity < 0.999) {
            if (root.glassRects.length > 0 || root.glassRegions.length > 0) {
                root.publishedGlassFingerprint = "";
                root.glassRects = [];
                root.glassRegions = [];
            }
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
                        "polygon": []
                    });
                }
                walk(child);
            }
        };
        walk(root.glassRoot);
        const fingerprint = foundRects.map(function(r) {
            return Math.round(r.x) + "," + Math.round(r.y) + ","
                   + Math.round(r.width) + "," + Math.round(r.height);
        }).join(";");
        if (fingerprint === root.publishedGlassFingerprint)
            return;
        root.publishedGlassFingerprint = fingerprint;
        root.glassRects = foundRects;
        root.glassRegions = foundRegions;
    }

    function scheduleGlassCollection() {
        if (!root.retiring)
            Qt.callLater(root.collectGlass);
    }

    onXChanged: root.scheduleGlassCollection()
    onYChanged: root.scheduleGlassCollection()
    onSurfacePositionChanged: root.scheduleGlassCollection()
    onVisibleChanged: root.scheduleGlassCollection()
    onAttachmentPendingChanged: root.scheduleGlassCollection()

    // ── The paint ─────────────────────────────────────────────────────────
    Item {
        id: content
        objectName: "celestina-soft-menu-content"

        anchors.fill: parent
        opacity: root.presentationOpacity
        Behavior on opacity {
            enabled: root.animateReveal && !root.reducedMotion
            NumberAnimation {
                duration: CelestinaTheme.motionExit
                easing.type: CelestinaTheme.easeStandard
            }
        }
        // The fade's two ends are the region's two edges: published when the
        // paint lands whole, withdrawn the moment it starts to leave.
        onOpacityChanged: root.scheduleGlassCollection()

        Rectangle {
            objectName: "celestina-backdrop-dim"

            // The whole carrier, not the field: the field is the card's
            // footprint, and the scrim must reach every output edge.
            parent: content
            x: -root.x
            y: -root.y
            width: root.Window.window ? root.Window.window.width : 0
            height: root.Window.window ? root.Window.window.height : 0
            visible: root.dimsBackdrop
            z: -2
            color: CelestinaTheme.backdropDim
        }

        CelestinaShadow {
            anchors.fill: parent
            visible: root.castsShadow
            radius: root.panelRadius
        }

        Item {
            id: body

            anchors.fill: parent
        }
    }
}
