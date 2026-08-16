// A card the host places at a point, inside a surface that covers the output.
//
// This is the placement and dismissal contract, and it is not decorative:
// `placeCard` in `panelmenucontroller.cpp` writes `menuX` and `menuY` back.
// That contract once had three hand-written copies and no owner; it has one
// here, and it is deliberately spelled in terms of a *card* rather than a menu,
// because a menu is only one of the things a panel control can open. A board of
// window tiles is another, and it must not arrive with a second copy of this
// arithmetic.
//
// What a consumer owes this file: `contentWidth` and `contentHeight`. They are
// not derived from the children, because the right measure differs by content
// and getting it wrong is not a rendering nuisance but a layout loop — a `Popup`
// fits itself to its window, so a window that sized itself to the laid-out popup
// shrank both of them by one margin per pass until the surface was a sliver.
// A consumer names the measure that is stable for the thing it draws.
//
// Why the surface is the whole output rather than the size of the card: a press
// anywhere outside the card is then this surface's to answer, which closes it in
// one click instead of leaving it up for whatever surface the press landed on.
// The layer surface itself is the input barrier.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window
import "EdgeAttachedGeometry.js" as EdgeAttachedGeometry

Window {
    id: root

    required property string outputName
    required property bool reducedMotion
    // Whatever this card is built from. Children land in the scene below and
    // position themselves at `cardX`, `cardY`.
    default property alias content: scene.data

    // The measures of the thing being carried, named by whoever draws it.
    property int contentWidth: 0
    property int contentHeight: 0
    // Zero keeps the historical natural-height behavior. A panel-relative,
    // model-backed card receives the actual remaining output height from its
    // controller and scrolls inside that viewport instead of moving its top.
    property int maximumContentHeight: 0
    property bool preserveRequestedTop: false
    // A real panel opener is optional. Panel-owned controls publish both its
    // exact output-local control rectangle and its icon anchor; point-only
    // routes such as workspace and foreign tray menus leave this false and
    // retain the established floating card.
    property alias anchoredFromPanel: placement.anchoredFromPanel
    property alias openerRect: placement.openerRect
    property alias attachmentAnchorRect: placement.attachmentAnchorRect
    property alias attachmentStartY: placement.attachmentStartY
    // A child menu born from a row of another menu keeps floating placement
    // but attaches its membrane sideways. The host places this card-sized
    // surface flush against the parent card and reserves the gap inside this
    // window; the card body sits beside that transparent strip.
    property bool attachedToMenuSide: false
    property bool attachmentSideRight: false
    // The membrane's horizontal travel reuses the same width-proportional
    // rule as the panel attachment gap, so parent-to-child distance and
    // bar-to-menu distance follow one vocabulary.
    // Unconditional on purpose: every consumer already gates on
    // `attachedToMenuSide` itself, and a gap that read the flag was stale at
    // exactly the wrong moment — inside the flag's own change handler, where
    // the surface size is recomputed and then read back synchronously by the
    // host, this binding had not re-evaluated yet, so the window was widened
    // by zero and the two menus were glued together with no membrane between
    // them.
    readonly property int sideAttachmentGap:
            Math.round(EdgeAttachedGeometry.proportionalMetric(
                cardWidth,
                CelestinaTheme.compEdgeAttachmentGapRatio,
                CelestinaTheme.compEdgeAttachmentGapMin,
                CelestinaTheme.compEdgeAttachmentGapMax))
    // What a glass card samples to blur. Named rather than reached for, so a
    // consumer never has to know the id of an item inside this file.
    readonly property Item backdrop: scene

    readonly property int anchorGap: placement.anchorGap
    // Where the host wants this card, in the surface's own coordinates. Named
    // `menuX`/`menuY` because that is what the host writes; renaming them would
    // be renaming an inter-object contract for tidiness.
    property int menuX: 0
    property int menuY: 0

    readonly property int cardWidth: contentWidth
    readonly property int cardHeight: maximumContentHeight > 0
                                      ? Math.min(contentHeight,
                                                 maximumContentHeight)
                                      : contentHeight
    // A card near an edge stays whole. Before the compositor has sized the
    // surface these clamp to zero, which is exactly where a card-sized surface
    // used to put it.
    // The complete visible card stays inside the surface.
    readonly property int cardX: Math.round(placement.x)
    // Clamped at the seam: before the placement's inputs settle it reads 0,
    // and a card at 0 on a full-output window sits over the bar — the frames
    // the author's recording caught on every menu open.
    readonly property int cardY: Math.max(Math.round(placement.y),
                                          Math.max(0, root.attachmentStartY))

    // Raised once the surface exists and the theme has been told about reduced
    // motion. A consumer opens its content from here rather than from its own
    // `Component.onCompleted`: both would be handlers for the same attached
    // signal on the same object, so the consumer's would silently replace this
    // file's and the reduced-motion route would never be applied.
    signal ready()
    signal dismissed()

    // How much larger this output needs the shell drawn; see shellscale.h. The
    // host supplies it and also divides the geometry it hands this card by it,
    // so every number below — the card, the opener it centres on, the seam a
    // membrane starts at — is already in these unscaled units and only the
    // last step to real pixels differs per monitor.
    property real shellScale: 1.0
    // The surface in those same units. A card-covering menu is sized from
    // here; an output-covering one is resized by the compositor and reads its
    // real size back through the same division.
    readonly property real surfaceWidth: root.width / root.shellScale
    readonly property real surfaceHeight: root.height / root.shellScale

    color: CelestinaTheme.clear

    // The size request, recomputed only on the host's own inputs. It re-runs
    // when the host caps the viewport or turns on the side membrane — both
    // happen right after creation, before the surface is mapped and read —
    // and deliberately never on content growth: a measured content height
    // settles after first layout, and re-requesting then is how the window
    // shrank under the surface that owns it.
    function requestSurfaceSize() {
        width = Math.round((cardWidth
                + (attachedToMenuSide ? Math.max(0, sideAttachmentGap) : 0))
                * shellScale);
        height = Math.round(cardHeight * shellScale);
    }

    onMaximumContentHeightChanged: root.requestSurfaceSize()
    onAttachedToMenuSideChanged: root.requestSurfaceSize()

    // One handler on purpose — a second `Component.onCompleted` on the same
    // object silently replaces the first, which is the exact trap the comment
    // above `ready()` warns consumers about.
    Component.onCompleted: {
        root.requestSurfaceSize();
        CelestinaTheme.reducedMotion = root.reducedMotion;
        root.ready();
    }

    PanelPopupPlacement {
        id: placement

        surfaceWidth: root.surfaceWidth
        surfaceHeight: root.surfaceHeight
        contentWidth: root.cardWidth
        contentHeight: root.cardHeight
        fallbackX: root.menuX
        fallbackY: root.menuY
        edgeInset: 0
        preserveRequestedTop: root.preserveRequestedTop
    }

    // Everything this card draws, in unscaled units, scaled once on its way to
    // the output. Scaling here rather than resizing every token keeps the
    // layout numbers the design states — and `CelestinaTheme` is a singleton
    // shared by every simultaneously mapped surface, so it could not carry a
    // per-output size even if that were wanted.
    Item {
        id: scene
        objectName: "celestina-shell-scene"

        width: root.surfaceWidth
        height: root.surfaceHeight
        transformOrigin: Item.TopLeft
        scale: root.shellScale
    }

}
