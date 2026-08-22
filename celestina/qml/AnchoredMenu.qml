// A menu drawn on an anchored card.
//
// Four menus are drawn this way — the panel's own, a tray item's, and the two
// connectivity indicators'. What they share with every other anchored surface —
// the clamp that keeps a card whole against an edge and the `menuX`/`menuY`
// contract the host writes through — lives in `AnchoredCard`. What is left here
// is what makes this a *menu*: the popup itself, its lifecycle, and the `menu`
// handle a consumer feeds items through.
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

    // Raised while the host parks this carrier for reuse (SURF-1-D). The
    // park closes the popup so its rows are really gone from the resting
    // scene, and that close must not read as the person dismissing the menu:
    // announced, it would start the retirement beat against a window that is
    // already being put away — the reentrancy the dismissal wiring below
    // guards against. Families that never park leave this false forever.
    property bool parkingForReuse: false

    // Height comes from the complete item model plus Menu padding, never from
    // the ListView's laid-out viewport, for the reason `AnchoredCard` records.
    // Reading ListView.contentHeight still lets Qt briefly feed the explicit
    // viewport height back through its implicit-height calculation while the
    // popup opens. Summing the rows' implicit measures keeps the two axes of
    // responsibility independent: rows own natural height; the card owns the
    // bounded viewport. Width comes from the menu's fixed token.
    readonly property int naturalMenuHeight: {
        let measured = menu.topPadding + menu.bottomPadding;
        for (let index = 0; index < menu.count; ++index) {
            const item = menu.itemAt(index);
            if (!item)
                continue;
            measured += item.implicitHeight;
        }
        return Math.ceil(measured);
    }
    contentWidth: menu.width
    contentHeight: root.naturalMenuHeight

    // Opened from the card's own signal rather than from a second
    // `Component.onCompleted`, which would replace the base file's handler —
    // and deferred one tick, because the host dresses a child window after
    // creating it, synchronously but later in the same call: the side
    // attachment, its anchor and the viewport cap all arrive between this
    // signal and the event loop. Opened immediately, the popup started its
    // stock enter transition before the attachment existed, so the rows
    // zoomed in on their own while the glass pushed sideways as a separate
    // piece. One tick later the attachment is complete, the transition is
    // suppressed for attached routes, and the surface moves as one thing.
    // Nothing has been drawn yet either way: the deferral resolves before
    // the first frame.
    //
    // Re-checked at fire time: between the queue and the tick the host can
    // park this carrier — a fast second click — and an open that lands then
    // leaves the popup visible inside the resting scene. The next resume's
    // replay finds it already open, gets no aboutToShow, and the carrier
    // comes back mapped and input-live with nothing painted.
    onReady: Qt.callLater(function() {
        if (!root.parkingForReuse)
            menu.open();
    })

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
        // In the card's own unscaled units, exactly like the glass. This popup
        // lands in the scaled scene through the card's default property, and a
        // Popup is positioned relative to its parent item *through* that
        // parent's transforms — so the mapping to output pixels already
        // happens once, in the scene. Scaling these coordinates here as well
        // is the double-scale that displaced every menu's rows on a 1.15
        // output while its glass stayed put.
        x: root.cardX
        y: root.cardY
        // `implicitHeight` remains the complete model height used by
        // AnchoredCard. `height` is the visible viewport and may be capped for a
        // panel-relative dynamic menu; Menu's ListView keeps the rest reachable.
        height: root.cardHeight
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        // The shared margin keeps a menu clear of the edges of the window it
        // pops up in. This surface exists only to carry this menu, so that clamp
        // has nothing to protect and only fights the size above.
        margins: -1
        // The host begins the one shared departure while the popup and its
        // rows still exist. Emitting from `closed` made the popup finish its
        // private exit first and only then started the carrier/glass exit,
        // producing two visibly separate closures in tray and phone menus.
        // A park-driven close is the one exception: the host is already
        // putting the carrier away, and announcing a dismissal from inside
        // that would start a second retirement against it.
        onAboutToHide: {
            if (!root.parkingForReuse)
                root.dismissed();
        }
    }

    // The factor is carried by the rows' own layer rather than by the popup.
    //
    // The popup cannot carry it: `GlassContextMenu` animates `scale` in both its
    // enter and exit transitions, and a transition writes the property directly,
    // so a binding on it is destroyed the first time the menu opens and the
    // rows snap back to unscaled. That was tried and is what the scaled-output
    // case caught.
    //
    // The content layer is free of that motion. Its clip is applied in its own
    // coordinates, before this transform, so a scaled list clips to the scaled
    // rectangle and no row is cut off by growing. The popup's own box stays
    // unscaled underneath, which is invisible: this surface paints its glass
    // itself and the popup's plate is hidden.
    Binding {
        target: menu.contentItem
        property: "scale"
        value: root.shellScale
    }

    // From the same corner the scene scales from. About the centre, the rows
    // would drift off their glass as they grew.
    Binding {
        target: menu.contentItem
        property: "transformOrigin"
        value: Item.TopLeft
    }

}
