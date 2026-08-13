// PANEL-1. A real Qt Quick menu in the panel's visual language.
//
// `AnchoredMenu` keeps the established Qt Quick Menu lifecycle: Escape,
// arrows, Enter, focus and outside-click dismissal. This component changes
// only its visual field. The old card plate is hidden; one transparent shared
// glass card and its denser internal sections replace independent row pills.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

AnchoredMenu {
    id: root

    property bool compositorBlurAvailable: false
    property alias glassRects: field.glassRects
    property alias glassRegions: field.glassRegions
    readonly property BackdropInk ink: backdropInk
    // Kept at zero by default so each menu chooses its own information
    // density. Connectivity and tray menus opt into a three-part rhythm:
    // transparent space after each row, a larger break after the header card,
    // and vertical air inside each body row. Qt's real Menu remains the
    // stacking and keyboard owner; SoftMenuRow owns the visible gaps because
    // Control.spacing does not space Menu delegates.
    property int itemSpacing: 0
    property int headerBodyGap: 0
    property int rowVerticalInset: 0
    readonly property int preferredWidth: CelestinaTheme.compMenuWidth
                                          + CelestinaTheme.space3xl * 3
    readonly property int headerRowHeight: CelestinaTheme.rowHeight
                                            + CelestinaTheme.borderFocus
                                            + CelestinaTheme.spaceSm

    BackdropInk {
        id: backdropInk
    }

    function scheduleGlassCollection() {
        field.scheduleGlassCollection();
    }

    // Keep the established shared Menu for focus and keyboard semantics, but
    // suppress its painted plate and capture: this surface supplies the one
    // compositor-backed field itself.
    Binding {
        target: root.menu.background
        property: "visible"
        value: false
    }

    Binding {
        target: root.menu
        property: "width"
        value: root.preferredWidth
    }

    Binding {
        target: root.menu.background
        property: "captureEnabled"
        value: false
    }

    Binding {
        target: root.menu.background
        property: "elevation"
        value: 0
    }

    // A real Menu keeps its rows in its own popup, not in the field's content
    // layer, so the field's internal ride cannot reach them. The body keeps
    // its complete size at every frame now and only its distance from the
    // seam moves, so following it is pure translation: the card and
    // everything in it falls from the bar and bounces with the glass, and
    // nothing is ever scaled or reflowed. Three earlier treatments — the
    // popup shown unclipped at the drop's mouth, a fade-in at the
    // destination, and an affine ride that scaled the card with the growing
    // body — were each rejected from the author's recordings.
    //
    // The offsets resolve to the stock values by construction once the drop
    // has settled, rather than by arithmetic that happens to come out at
    // zero: a live menu keeps changing height under the ride, because the
    // performance readings rebuild their complete row list on every tick.
    readonly property bool ridesTheDrop: field.attachmentClipsContent

    Binding {
        target: root.menu
        property: "x"
        // In unscaled units like everything else: the popup's parent is the
        // scaled scene and the mapping to output pixels happens there, once.
        value: root.cardX + (root.ridesTheDrop
                             ? field.attachmentBodyRect.x : 0)
        restoreMode: Binding.RestoreBindingOrValue
    }

    Binding {
        target: root.menu
        property: "y"
        value: root.cardY + (root.ridesTheDrop
                             ? field.attachmentBodyRect.y : 0)
        restoreMode: Binding.RestoreBindingOrValue
    }

    Binding {
        target: root.menu
        property: "opacity"
        value: field.attachmentContentOpacity
        restoreMode: Binding.RestoreBindingOrValue
    }

    // The stock popup enter motion (its small scale-up and fade at the final
    // position) is replaced by the drop itself on attached routes. Leaving it
    // on gave the author's recording its bug: Menu emits `opened()` only when
    // that enter transition has finished, so the card showed complete and
    // settled first and the fall replayed afterwards.
    Binding {
        target: root.menu
        property: "enter"
        value: null
        when: field.edgeAttachmentRequested
    }

    SoftMenuField {
        id: field

        // The glass is placed by the card, not by the popup that rides the
        // drop; its own silhouette already carries the motion.
        x: root.cardX
        y: root.cardY
        width: root.contentWidth
        height: root.cardHeight
        reducedMotion: root.reducedMotion
        ink: root.ink
        animateReveal: false
        compositorBlurAvailable: root.compositorBlurAvailable
        attachedToTop: root.anchoredFromPanel
        openerRect: root.openerRect
        attachmentAnchorRect: root.attachmentAnchorRect
        attachmentStartY: root.attachmentStartY
        attachedToSide: root.attachedToMenuSide
        attachmentSideRight: root.attachmentSideRight
        sideAttachmentGap: root.sideAttachmentGap
        surfacePosition: Qt.point(root.cardX, root.cardY)

        // The first non-focusable row paints the heading section. The body is
        // the second continuous group behind the remaining rows; no row adds a
        // compositor sample or a resting pill of its own.
        MenuSection {
            anchors.leftMargin: CelestinaTheme.compMenuPadding
            anchors.rightMargin: CelestinaTheme.compMenuPadding
            anchors.topMargin: CelestinaTheme.compMenuPadding
                               + root.headerRowHeight
                               + root.headerBodyGap
            anchors.bottomMargin: CelestinaTheme.compMenuPadding
            ink: root.ink
        }
    }

    Connections {
        target: root.menu

        // `aboutToShow` starts the fall before the popup's first visible
        // frame; Menu emits `opened` only after its enter transition, which
        // is far too late and remains only as a fallback. Both are
        // idempotent.
        function onAboutToShow() {
            field.reveal();
        }

        function onOpened() {
            field.reveal();
        }

        // The host destroys this window once the popup reports itself closed,
        // and Menu emits that only after its exit transition. Retiring the
        // glass here is what keeps it from outliving the rows it carries by
        // the width of that transition.
        function onAboutToHide() {
            field.retire();
        }

        function onCountChanged() {
            field.scheduleGlassCollection();
        }
    }
}
