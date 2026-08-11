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
    // What a glass card samples to blur. Named rather than reached for, so a
    // consumer never has to know the id of an item inside this file.
    readonly property Item backdrop: scene

    readonly property int anchorGap: CelestinaTheme.compFloatingGap
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
    readonly property int cardY: Math.round(placement.y)

    // Raised once the surface exists and the theme has been told about reduced
    // motion. A consumer opens its content from here rather than from its own
    // `Component.onCompleted`: both would be handlers for the same attached
    // signal on the same object, so the consumer's would silently replace this
    // file's and the reduced-motion route would never be applied.
    signal ready()
    signal dismissed()

    width: cardWidth
    height: cardHeight
    color: CelestinaTheme.clear

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = root.reducedMotion;
        root.ready();
    }

    PanelPopupPlacement {
        id: placement

        surfaceWidth: root.width
        surfaceHeight: root.height
        contentWidth: root.cardWidth
        contentHeight: root.cardHeight
        fallbackX: root.menuX
        fallbackY: root.menuY
        edgeInset: 0
        preserveRequestedTop: root.preserveRequestedTop
    }

    Item {
        id: scene

        anchors.fill: parent
    }

}
