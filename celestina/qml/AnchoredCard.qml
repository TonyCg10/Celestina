// A card the host places at a point, inside a surface that covers the output.
//
// This is the placement and dismissal contract, and it is not decorative:
// `placeCard` in `panelmenucontroller.cpp` reads `shadowMargin` off the window
// and writes `menuX` and `menuY` back. That contract once had three hand-written
// copies and no owner; it has one here, and it is deliberately spelled in terms
// of a *card* rather than a menu, because a menu is only one of the things a
// panel control can open. A board of window tiles is another, and it must not
// arrive with a second copy of this arithmetic.
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

    required property bool reducedMotion
    // Whatever this card is built from. Children land in the scene below, and
    // position themselves at `cardX + shadowMargin`, `cardY + shadowMargin`.
    default property alias content: scene.data

    // The measures of the thing being carried, named by whoever draws it.
    property int contentWidth: 0
    property int contentHeight: 0
    // What a glass card samples to blur. Named rather than reached for, so a
    // consumer never has to know the id of an item inside this file.
    readonly property Item backdrop: scene

    // The card is inset from the surface by exactly the room its drop shadow
    // needs — `GlassSurface` draws the shadow outside the card and the window
    // would otherwise clip it. The host pulls the anchor back by this much so
    // the card, not the surface, lands where the click was.
    readonly property int shadowMargin: CelestinaTheme.shadowBlur
                                        + CelestinaTheme.shadowOffsetY
    // Where the host wants this card, in the surface's own coordinates. Named
    // `menuX`/`menuY` because that is what the host writes; renaming them would
    // be renaming an inter-object contract for tidiness.
    property int menuX: 0
    property int menuY: 0

    readonly property int cardWidth: contentWidth + shadowMargin * 2
    readonly property int cardHeight: contentHeight + shadowMargin * 2
    // A card near an edge stays whole. Before the compositor has sized the
    // surface these clamp to zero, which is exactly where a card-sized surface
    // used to put it.
    // The shadow may extend beyond the surface; the opaque card may not. A zero
    // minimum here inserted one complete shadow margin between an
    // exclusive-zone-aware surface edge and the visible card.
    readonly property int cardX: Math.max(-shadowMargin,
                                          Math.min(menuX,
                                                   root.width - cardWidth
                                                   + shadowMargin))
    readonly property int cardY: Math.max(-shadowMargin,
                                          Math.min(menuY,
                                                   root.height - cardHeight
                                                   + shadowMargin))

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

    Item {
        id: scene

        anchors.fill: parent
    }

}
