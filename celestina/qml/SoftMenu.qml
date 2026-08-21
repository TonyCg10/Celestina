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

    // This family's park and resume are complete (SURF-1-D): the popup that
    // owns its reveal and retirement is closed silently on park and replayed
    // on resume, so a resumed carrier presents through the popup's own gates
    // exactly as a fresh open — which is the promise this property makes to
    // the host. See `SoftCard` for the card family's declaration.
    readonly property bool carrierReusable: true

    // The park's half: the rows leave with the paint, without announcing a
    // dismissal the host did not ask for.
    function prepareForPark() {
        root.parkingForReuse = true;
        root.menu.close();
    }

    // The resume's half, invoked by the host once the attachment is
    // re-established. The same one-tick deferral as the first open's
    // `onReady`: the host dresses the window synchronously but later in the
    // same call, and a popup opened before the attachment exists starts its
    // enter transition detached. Reveal, glass and blur all follow from the
    // popup's own aboutToShow, as they always have.
    function reopenForReuse() {
        root.parkingForReuse = false;
        Qt.callLater(function() { root.menu.open(); });
    }

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

    // The entry offset is the top route's whole ride; zero at rest and on
    // every route that does not fall. The rows are a real popup the field's
    // seam clip cannot reach, so the clip is rebuilt on the popup itself:
    // the popup is pinned at the seam while the card is still behind the
    // bar, its rows are slid up by exactly the hidden distance inside its
    // own clipped viewport, and so what shows is the slice of the block
    // already past the seam — the same progressive emergence every card
    // menu's content gets, never a row drawn over the bar and never rows
    // waiting at the seam before their glass arrives, which read as a
    // second, different animation on the author's recording (2026-08-14).
    readonly property real rowsRideY:
            root.cardY + field.entryOffsetY
            + (root.ridesTheDrop ? field.attachmentBodyRect.y : 0)
    readonly property real rowsSeamY:
            root.anchoredFromPanel && field.attachmentStartY >= 0
            ? field.attachmentStartY : -1e9
    readonly property real rowsCut: Math.max(0, root.rowsSeamY - root.rowsRideY)
    // Popup.Item reparents the visual popup to the window Overlay. Its styled
    // content item is the fixed, clipped ListView viewport; Flickable's own
    // contentItem is the row carrier that may move inside that viewport.
    // Keeping both handles typed also makes a style that stops supplying the
    // required viewport fail closed instead of silently moving an unclipped
    // generic Item over the panel.
    readonly property Flickable rowsViewport:
            root.menu.contentItem as Flickable
    readonly property Item rowsContent:
            root.rowsViewport ? root.rowsViewport.contentItem : null

    Translate {
        id: rowsSlide

        y: -root.rowsCut
    }

    Binding {
        target: root.menu
        property: "y"
        value: Math.max(root.rowsSeamY, root.rowsRideY)
        restoreMode: Binding.RestoreBindingOrValue
    }

    // Move the rows, never their viewport. Moving `menu.contentItem` moved the
    // ListView's clip with it, so its effective top became
    // `rowsSeamY - rowsCut == rowsRideY` and the first falling frames painted
    // over the panel. The inner Flickable carrier preserves the ListView's
    // fixed seam clip, its current scroll position, and the scale applied by
    // AnchoredMenu. The zero translation is safe on every non-attached route,
    // so no conditional binding can expose the stock popup during bootstrap.
    Binding {
        target: root.rowsViewport
        property: "clip"
        value: true
        when: root.rowsViewport !== null
    }

    Binding {
        target: root.rowsContent
        property: "transform"
        value: rowsSlide
        when: root.rowsContent !== null
        restoreMode: Binding.RestoreBindingOrValue
    }

    Binding {
        target: root.menu
        property: "opacity"
        value: field.presentationOpacity
        // Popup.Item reparents these rows outside the field. Keep them behind
        // the same presentation gate on floating routes too; after reveal Qt
        // regains its stock popup opacity unless attachment or retirement
        // requires the shared lifecycle value.
        when: !field.revealed || field.edgeAttachmentRequested || field.retiring
        restoreMode: Binding.RestoreBindingOrValue
    }

    Binding {
        target: root.menu
        property: "scale"
        value: field.retireScale
        when: field.retiring
        restoreMode: Binding.RestoreBindingOrValue
    }

    Binding {
        target: root.menu
        property: "transformOrigin"
        value: Item.Center
        when: field.retiring
        restoreMode: Binding.RestoreBindingOrValue
    }

    // The field owns the attached entry and every departure. A floating menu
    // retains the stock popup entry so Qt also retains its focus lifecycle.
    // During departure the transition is only a lifetime hold: popup rows sit
    // outside the field's item tree, so the bindings above mirror the field's
    // opacity and scale while Qt keeps them alive. A transition that also
    // wrote opacity/scale destroyed those bindings and was the second visual
    // clock behind the split close.
    Transition {
        id: departureHold

        PauseAnimation {
            duration: root.reducedMotion ? 0 : CelestinaTheme.motionFast
        }
    }

    Binding {
        target: root.menu
        property: "enter"
        value: null
        when: field.edgeAttachmentRequested
    }

    Binding {
        target: root.menu
        property: "exit"
        value: departureHold
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

        function onCountChanged() {
            field.scheduleGlassCollection();
        }
    }
}
