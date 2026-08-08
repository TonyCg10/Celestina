// A menu drawn on an anchored card.
//
// Four menus are drawn this way — the panel's own, a tray item's, and the two
// connectivity indicators'. What they share with every other anchored surface —
// the shadow inset, the clamp that keeps a card whole against an edge, and the
// `shadowMargin`/`menuX`/`menuY` contract the host writes through — lives in
// `AnchoredCard`. What is left here is what makes this a *menu*: the popup
// itself, its lifecycle, and the `menu` handle a consumer feeds items through.
//
// `GlassContextMenu` supplies Escape, arrow keys, focus, and motion that already
// honours `reducedMotion`.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Controls

AnchoredCard {
    id: root

    // Exposed so a consumer can insert items into the real menu. Reading it is
    // how an `Instantiator` reaches `insertItem`, which is the only way to feed
    // a `Menu` a model.
    readonly property alias menu: menu

    // Height comes from what the menu's content *implies*, never from the menu's
    // own laid-out height, for the reason `AnchoredCard` records. Width comes
    // from the menu, because the shared component fixes it to a token rather
    // than deriving it from anything here.
    contentWidth: menu.width
    contentHeight: menu.implicitHeight

    // Opened from the card's own signal rather than from a second
    // `Component.onCompleted`, which would replace the base file's handler.
    onReady: menu.open()

    GlassContextMenu {
        id: menu

        // Nothing of the shell is behind this surface — the compositor's own
        // backdrop is. The glass therefore reads as its tint here.
        backdropSource: root.backdrop
        // The layer window already prevents an outside click from landing on an
        // application. Non-modal is what lets a click in the panel's reserved
        // strip reach a different indicator and replace this menu in the same
        // gesture.
        modal: false
        x: root.cardX + root.shadowMargin
        y: root.cardY + root.shadowMargin
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        // The shared margin keeps a menu clear of the edges of the window it
        // pops up in. This surface exists only to carry this menu, so that clamp
        // has nothing to protect and only fights the size above.
        margins: -1
        onClosed: root.dismissed()
    }

}
