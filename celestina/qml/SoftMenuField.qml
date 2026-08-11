// The visual field shared by every PANEL-1 interactive menu surface.
//
// One nearly transparent compositor-backed card owns the menu. Slightly denser
// neutral sections divide its content without publishing more blur regions or
// adding an exterior halo.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: root

    required property bool reducedMotion
    required property BackdropInk ink
    default property alias contentData: content.data

    property bool animateReveal: true
    property bool revealed: false
    property bool compositorBlurAvailable: false
    property Item glassRoot: content
    property var glassRects: []
    property var glassRegions: []

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
                    foundRegions.push({"rect": rect, "radius": child.radius});
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

    Timer {
        id: glassSettle

        // The rectangle must be sampled after the scale-up reaches 1.0;
        // publishing its transformed origin with its untransformed size midway
        // through the animation creates a blur region larger than the card.
        interval: root.animateReveal && !root.reducedMotion
                  ? CelestinaTheme.motionNormal + CelestinaTheme.space3xl
                  : 80
        repeat: false
        onTriggered: root.collectGlass()
    }

    Item {
        id: content

        anchors.fill: parent
        transformOrigin: Item.Top
        scale: !root.animateReveal || root.revealed || root.reducedMotion ? 1 : 0.92
        opacity: !root.animateReveal || root.revealed || root.reducedMotion ? 1 : 0

        CompositorGlassRegion {
            anchors.fill: parent
            z: -3
            blurAvailable: root.compositorBlurAvailable
            // A missing compositor sample still needs a contrast floor. The
            // live blur path below deliberately does not reuse this dark
            // fallback tint.
            fallbackColor: CelestinaTheme.glassTint
            radius: CelestinaTheme.radiusMd
            onBlurRegionChanged: root.scheduleGlassCollection()
        }

        // The compositor or the region's fallback supplies the external
        // backdrop. Style remains the one material authority for this very
        // light veil and for every denser section above it.
        GlassSurface {
            anchors.fill: parent
            z: -2
            objectName: "celestina-menu-body-tint"
            backdropMode: GlassSurface.ExternalBackdrop
            externalBackdropReady: true
            captureEnabled: false
            materialRole: GlassSurface.ContextualVeil
            materialTint: root.ink.materialTint
            cornerRadius: CelestinaTheme.radiusMd
            elevation: 0
        }

        Behavior on scale {
            enabled: !root.reducedMotion

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
