// A menu card the host places inside a surface that covers the whole output.
//
// Four menus are drawn this way — the panel's own, a tray item's, and the two
// connectivity indicators' — and the contract they share is not decorative:
// `placeCard` in `panelmenucontroller.cpp` reads `shadowMargin` off the window
// and writes `menuX` and `menuY` back. That contract had three hand-written
// copies and no owner. It has one here.
//
// Why the surface is the whole output rather than the size of the card: a press
// anywhere outside the card is then this menu's to answer, which is what closes
// it in one click instead of leaving it up for whatever surface the click
// landed on. The layer surface itself is the input barrier; making its nested
// `Menu` modal would consume a press on another panel control before that
// control can replace this menu. `GlassContextMenu` supplies Escape, arrow
// keys, focus, and motion that already honours `reducedMotion`.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Controls
import QtQuick.Window

Window {
    id: root

    required property bool reducedMotion
    // Whatever the menu is built from goes here; consumers add their items to
    // `menu` through it.
    default property alias content: scene.data
    // Exposed so a consumer can insert items into the real menu. Reading it is
    // how an `Instantiator` reaches `insertItem`, which is the only way to feed
    // a `Menu` a model.
    readonly property alias menu: menu

    // The card is inset from the surface by exactly the room its drop shadow
    // needs — `GlassSurface` draws the shadow outside the card and the window
    // would otherwise clip it. The host pulls the anchor back by this much so
    // the card, not the surface, lands where the click was.
    readonly property int shadowMargin: CelestinaTheme.shadowBlur
                                        + CelestinaTheme.shadowOffsetY
    // Where the host wants this card, in the surface's own coordinates.
    property int menuX: 0
    property int menuY: 0

    readonly property int cardWidth: menu.width + shadowMargin * 2
    readonly property int cardHeight: menu.implicitHeight + shadowMargin * 2
    // A menu near an edge stays whole. Before the compositor has sized the
    // surface these clamp to zero, which is exactly where a card-sized surface
    // used to put it.
    // The shadow may extend beyond the surface; the opaque menu may not. A
    // zero minimum here inserted one complete shadow margin between an
    // exclusive-zone-aware surface edge and the visible card.
    readonly property int cardX: Math.max(-shadowMargin,
                                         Math.min(menuX,
                                                  root.width - cardWidth
                                                  + shadowMargin))
    readonly property int cardY: Math.max(-shadowMargin,
                                         Math.min(menuY,
                                                  root.height - cardHeight
                                                  + shadowMargin))

    signal dismissed()

    // Height comes from what the menu's content *implies*, never from the
    // menu's own laid-out height: `Popup` fits itself to its window, so a window
    // that sized itself to the laid-out popup shrank both of them by one margin
    // per pass until the surface was a sliver. Width comes from the menu,
    // because the shared component fixes it to a token rather than deriving it
    // from anything here.
    width: cardWidth
    height: cardHeight
    color: CelestinaTheme.clear

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = root.reducedMotion;
        menu.open();
    }

    Item {
        id: scene

        anchors.fill: parent

        GlassContextMenu {
            id: menu

            // Nothing of the shell is behind this surface — the compositor's
            // own backdrop is. The glass therefore reads as its tint here.
            backdropSource: scene
            // The layer window already prevents an outside click from landing
            // on an application. Non-modal is what lets a click in the panel's
            // reserved strip reach a different indicator and replace this menu
            // in the same gesture.
            modal: false
            x: root.cardX + root.shadowMargin
            y: root.cardY + root.shadowMargin
            closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
            // The shared margin keeps a menu clear of the edges of the window it
            // pops up in. This surface exists only to carry this menu, so that
            // clamp has nothing to protect and only fights the size above.
            margins: -1
            onClosed: root.dismissed()
        }

    }

}
