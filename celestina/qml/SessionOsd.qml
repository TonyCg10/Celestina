// The on-screen display: what a device is at, in the corner, for a moment.
//
// It shows readings, never requests. The host hands it values a provider
// actually published, so a key that changed nothing raises nothing here, and a
// bar is drawn only when there is a level to draw — a device that reported no
// level says so in words instead of showing an empty bar at zero.
//
// It is a card file, not a single card: a volume change while a brightness
// card is still up adds a second card behind the first, offset so its header
// stays readable, because the overwritten number was information someone was
// looking at. Hovering a card behind raises it to the front. That hover is the
// one interaction this surface has — nothing here is a control, and the host
// masks everything above the bar's seam so the wheel that raised the display
// keeps stepping the control under it.
//
// PANEL-1. Each card is the same glass as the bar's readings and their menus:
// one compositor-backed veil (`SoftMenuField`) carrying one dense content
// section, published to the blur controller through `glassRegions`. The front
// card attaches to the bar with the same drop membrane every menu uses, its
// mouth on the panel icon of the reading it shows.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: osd

    // The front card, as the host has always announced it. `readings` below
    // carries the whole file; these four remain the compatibility contract
    // and the source for the synthesized single card when the host sets only
    // them.
    required property string kind
    // Whole percent, or negative when the provider reported no level.
    required property int percent
    required property bool muted
    // Which device this is about, when the session has more than one of them.
    required property string label
    required property bool reducedMotion

    // The whole card file, front first: a list of {kind, percent, muted,
    // label} maps. `var` is necessary: QML has no typed map-list.
    property var readings: []

    // The compositor sample the cards draw over. `OverlaySurface` arms a
    // `PanelBlurController` for any window that publishes these, and the
    // controller writes `compositorBlurAvailable` back when the effect took.
    // The file is one surface with several cards, so it publishes the union
    // of their regions, exactly as the toast stack does.
    property bool compositorBlurAvailable: false
    property var glassRects: []
    property var glassRegions: []

    // The front card appears attached to the bar, a drop out of the panel
    // icon of the reading it shows — the same contract every menu carries.
    // The host resolves the rectangles because a reading changes without a
    // click; all of them arrive in output-local shell units. Left at their
    // defaults the display is the floating file, which is also the fallback
    // the host uses when the top-right zone is already taken.
    property bool anchoredFromPanel: false
    property rect openerRect: Qt.rect(0, 0, 0, 0)
    property rect attachmentAnchorRect: Qt.rect(0, 0, 0, 0)
    property real attachmentStartY: -1
    // Where this window sits on its output, and how large it is, in shell
    // units. An attached window is wider than its card: it spans from the
    // leftmost thing it must contain — card or icon — to the output's right
    // edge, so the membrane's mouth is never clipped by its own window.
    property real surfaceOriginX: 0
    property real surfaceWidth: osd.cardWidth
    property real surfaceHeight: osd.neededHeight
    // How much larger this output needs the shell drawn; see shellscale.h.
    property real shellScale: 1.0
    // The fallback window sits flush with the screen's bottom edge and its
    // card enters by physically emerging from it: fast off the edge, braking
    // into place, no recoil — an arrival, not a landing.
    property bool entersFromBottom: false

    readonly property int cardWidth: 260
    readonly property int cardHeight: 96
    // How much of a card behind stays visible under the one before it.
    // Pinned by the host's own constant of the same name.
    readonly property int stackPeek: 28

    readonly property bool hasLevel: percent >= 0

    // One vocabulary for every card, front or behind. The bar shows these
    // same three quantities under these same glyphs, so the display names a
    // device the way the reading it came from does. An unknown kind is shown
    // by name rather than dropped, because a display that silently painted
    // nothing would look like a broken key.
    function headlineFor(kind, label) {
        if (kind === "volume")
            return qsTr("Volumen");
        if (kind === "microphone")
            return qsTr("Micrófono");
        if (kind === "brightness") {
            return label.length > 0 ? qsTr("Brillo — %1").arg(label)
                                    : qsTr("Brillo");
        }
        return kind;
    }
    function valueTextFor(percent, muted) {
        if (muted)
            return qsTr("Silenciado");
        if (percent < 0)
            return qsTr("Sin lectura");
        return qsTr("%1 %").arg(percent);
    }
    function iconFor(kind, muted) {
        if (kind === "volume")
            return muted ? "media-volume-muted" : "media-volume";
        if (kind === "microphone")
            return muted ? "mic-off" : "mic";
        if (kind === "brightness")
            return "sun";
        return "info";
    }

    // The front card's face, kept as the read-only contract it has always
    // been: what the tests pin and what the window announces.
    readonly property string headline: osd.headlineFor(osd.kind, osd.label)
    readonly property string valueText: osd.valueTextFor(osd.percent, osd.muted)
    readonly property string iconName: osd.iconFor(osd.kind, osd.muted)
    // What a screen reader is told, in one sentence: the same two facts the
    // eye gets from the title and the number.
    readonly property string spokenText: qsTr("%1: %2").arg(osd.headline).arg(osd.valueText)

    // The file itself: the host's list, or the single synthesized card when
    // only the four front properties were set (the compatibility route the
    // tests use). An empty kind with an empty list is the persistent window
    // resting between readings: nothing is drawn and nothing takes input,
    // but the surface stays mapped, because a freshly mapped layer window
    // measured up to two and a half seconds from creation to its first
    // presented frame on the nested session — longer than a card lives.
    readonly property var cards: osd.readings.length > 0
            ? osd.readings
            : osd.kind.length > 0
              ? [{"kind": osd.kind, "percent": osd.percent,
                  "muted": osd.muted, "label": osd.label}]
              : []

    // The file's own height: the front card plus one peek per card behind,
    // and the bottom-entry window keeps the breathing room below the card
    // that its flush margin gave up.
    readonly property real neededHeight: osd.cardHeight
            + Math.max(0, osd.cards.length - 1) * osd.stackPeek
            + (osd.entersFromBottom ? CelestinaTheme.spaceLg : 0)

    width: Math.round(osd.surfaceWidth * osd.shellScale)
    height: Math.round(
        (osd.anchoredFromPanel
         ? Math.max(osd.surfaceHeight, placement.y + osd.neededHeight
                    + CelestinaTheme.spaceLg)
         : osd.neededHeight) * osd.shellScale)
    color: CelestinaTheme.clear
    title: qsTr("Indicador en pantalla de Celestina")

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = osd.reducedMotion;
        osd.syncCards();
    }

    // The model is kept in place rather than rebuilt: a rebuilt list would
    // recreate every delegate on every wheel notch, and each recreation would
    // replay the reveal and the fall — a display that drips once per notch.
    ListModel {
        id: cardModel
    }

    // The kinds currently receding. A card leaves by moving away — fading
    // and shrinking — so its row outlives its reading by one exit beat; a
    // reading that returns during that beat simply reclaims the row.
    property var departingKinds: []

    function syncCards() {
        const list = osd.cards;
        let receding = [];
        for (let stale = cardModel.count - 1; stale >= 0; --stale) {
            let present = false;
            for (let index = 0; index < list.length; ++index) {
                if (list[index].kind === cardModel.get(stale).kind)
                    present = true;
            }
            if (!present) {
                if (osd.reducedMotion) {
                    cardModel.remove(stale);
                } else if (osd.departingKinds.indexOf(
                               cardModel.get(stale).kind) < 0) {
                    receding.push(cardModel.get(stale).kind);
                }
            }
        }
        if (receding.length > 0) {
            osd.departingKinds = osd.departingKinds.concat(receding);
            departureSweep.restart();
        }
        // A reading that came back mid-exit reclaims its row.
        if (osd.departingKinds.length > 0) {
            const stillLeaving = [];
            for (let gone = 0; gone < osd.departingKinds.length; ++gone) {
                let returned = false;
                for (let index = 0; index < list.length; ++index) {
                    if (list[index].kind === osd.departingKinds[gone])
                        returned = true;
                }
                if (!returned)
                    stillLeaving.push(osd.departingKinds[gone]);
            }
            if (stillLeaving.length !== osd.departingKinds.length)
                osd.departingKinds = stillLeaving;
        }
        for (let index = 0; index < list.length; ++index) {
            const entry = {
                "kind": list[index].kind,
                "percent": list[index].percent,
                "muted": list[index].muted === true,
                "label": list[index].label !== undefined ? list[index].label : ""
            };
            let at = -1;
            for (let have = 0; have < cardModel.count; ++have) {
                if (cardModel.get(have).kind === entry.kind)
                    at = have;
            }
            if (at < 0) {
                cardModel.insert(index, entry);
            } else {
                cardModel.set(at, entry);
                if (at !== index)
                    cardModel.move(at, index, 1);
            }
        }
    }

    // The sweep that finally removes a receded row, once its exit has had
    // its full beat on screen.
    Timer {
        id: departureSweep

        interval: CelestinaTheme.motionNormal
        onTriggered: {
            for (let stale = cardModel.count - 1; stale >= 0; --stale) {
                if (osd.departingKinds.indexOf(cardModel.get(stale).kind) >= 0)
                    cardModel.remove(stale);
            }
            osd.departingKinds = [];
            Qt.callLater(osd.collectGlass);
        }
    }

    onCardsChanged: {
        osd.syncCards();
        // A card leaving destroys its delegate, and a destroyed field emits
        // nothing — the union would keep the dead card's region and the blur
        // controller would never see it empty, which left an armed region
        // blurring a bare rectangle over the wallpaper after expiry. Collect
        // again once the removal has settled.
        Qt.callLater(osd.collectGlass);
    }

    // One surface, several cards: the union of what each collected, exactly
    // as the toast stack publishes its own.
    function collectGlass() {
        // The bottom-entry window publishes one static region while anything
        // is on it. Chasing the ride per frame put the double-buffered effect
        // one frame behind the card, which read as the backdrop darkening for
        // a few frames; and a region covering the whole runway window drew a
        // visible slab past the card's edges. The card's resting footprint is
        // the still point between the two: the entry slides up into glass
        // that is already there, and neither dimension exceeds the card.
        if (osd.entersFromBottom) {
            const occupied = osd.cards.length > 0
                             || osd.departingKinds.length > 0;
            const footprint = Qt.rect(
                0, 0,
                Math.round(osd.cardWidth * osd.shellScale),
                Math.round(osd.cardHeight * osd.shellScale));
            osd.glassRects = occupied ? [footprint] : [];
            osd.glassRegions = occupied
                ? [{"rect": footprint,
                    "radius": CelestinaTheme.radiusMd * osd.shellScale,
                    "polygon": []}]
                : [];
            return;
        }
        // An empty file publishes empty glass outright: the walk below races
        // delegate destruction — a dying card can still be in the tree when
        // the deferred recollect runs, and a union that kept it left the
        // armed region blurring bare wallpaper, sometimes, which is the worst
        // kind of defect to chase.
        if (osd.cards.length === 0) {
            osd.glassRects = [];
            osd.glassRegions = [];
            return;
        }
        const rects = [];
        const regions = [];
        // The cards live under the seam-law clip now, one level below the
        // scene; the walk follows them there.
        for (let index = 0; index < seamLawInterior.children.length; ++index) {
            const card = seamLawInterior.children[index];
            if (!card || card.glassRegions === undefined)
                continue;
            for (let each = 0; each < card.glassRegions.length; ++each)
                regions.push(card.glassRegions[each]);
            for (let rect = 0; rect < card.glassRects.length; ++rect)
                rects.push(card.glassRects[rect]);
        }
        osd.glassRects = rects;
        osd.glassRegions = regions;
    }

    BackdropInk {
        id: backdropInk
    }

    // The same placement rule every panel-opened surface uses, fed in this
    // window's own coordinates: the opener is translated by the window's
    // origin, and the seam keeps its output value because the window touches
    // the output's top edge.
    PanelPopupPlacement {
        id: placement

        surfaceWidth: osd.surfaceWidth
        surfaceHeight: osd.surfaceHeight
        contentWidth: osd.cardWidth
        contentHeight: osd.cardHeight
        anchoredFromPanel: osd.anchoredFromPanel
        openerRect: Qt.rect(osd.openerRect.x - osd.surfaceOriginX,
                            osd.openerRect.y,
                            osd.openerRect.width, osd.openerRect.height)
        attachmentAnchorRect: osd.attachmentAnchorRect
        attachmentStartY: osd.attachmentStartY
        fallbackX: 0
        fallbackY: 0
        edgeInset: osd.anchoredFromPanel ? CelestinaTheme.spaceSm : 0
    }

    // A display that is already up is updated in place by the host, so each
    // card reveals once, when it is created on a shown surface.
    onVisibleChanged: {
        if (!osd.visible)
            return;
        for (let index = 0; index < scene.children.length; ++index) {
            const card = scene.children[index];
            if (card && card.reveal !== undefined)
                Qt.callLater(card.reveal);
        }
    }

    // The persistent window's pulse. A Wayland window that commits nothing
    // stops receiving frame callbacks, and Qt then treats it as no longer
    // exposed — after which new content dirties a scene nobody renders, and
    // a card pushed onto the resting surface never reaches the screen. This
    // was measured, not theorised: the journal showed the push and the
    // compositor showed nothing. One invisible pixel changing twice a second
    // keeps one commit in flight, which keeps the callbacks flowing, which
    // keeps the window a window. The panel never needed this only because
    // its clock repaints every second anyway.
    Rectangle {
        id: heartbeat

        width: 1
        height: 1
        color: CelestinaTheme.clear

        // Moving it by a pixel is what dirties the scene graph; the colour is
        // transparent, so nothing about the surface's look depends on this.
        Timer {
            interval: 500
            running: osd.visible
            repeat: true
            onTriggered: {
                heartbeat.x = heartbeat.x === 0 ? 1 : 0;
                osd.requestUpdate();
            }
        }
    }

    // Every card lives inside a scene item that carries the per-output
    // factor, exactly as the panel's own scene does.
    Item {
        id: scene

        width: osd.surfaceWidth
        height: osd.anchoredFromPanel ? osd.surfaceHeight : osd.neededHeight
        transformOrigin: Item.TopLeft
        scale: osd.shellScale

        // The paint law of the attached window, enforced where every card
        // lives: nothing may exist above the seam. The membrane's mouth is
        // tangent at the seam and paints downward, so it loses nothing; the
        // card that slipped above it — whatever geometry raced to put it
        // there — simply does not show. The bottom window has no bar and no
        // seam, and keeps its full canvas.
        Item {
            id: seamLaw

            x: 0
            y: !osd.entersFromBottom && osd.attachmentStartY >= 0
               ? osd.attachmentStartY : 0
            width: scene.width
            height: scene.height - seamLaw.y
            clip: seamLaw.y > 0

            Item {
                id: seamLawInterior

                x: 0
                y: -seamLaw.y
                width: scene.width
                height: scene.height

        Repeater {
            model: cardModel

            delegate: SoftMenuField {
                id: card

                required property int index
                required property string kind
                required property int percent
                required property bool muted
                required property string label

                readonly property string cardHeadline:
                    osd.headlineFor(card.kind, card.label)
                readonly property string cardValueText:
                    osd.valueTextFor(card.percent, card.muted)
                readonly property bool cardHasLevel: card.percent >= 0
                readonly property string cardSpokenText: qsTr("%1: %2")
                    .arg(card.cardHeadline).arg(card.cardValueText)

                readonly property bool departing:
                        osd.departingKinds.indexOf(card.kind) >= 0

                x: Math.round(placement.x)
                // Clamped at the seam for every top route, whatever the
                // routing knows so far: before `anchoredFromPanel` settles
                // this read 0, and 0 on the attached window is the screen's
                // top edge — the author's recording caught the finished card
                // over the bar's own icons for exactly those frames.
                y: (osd.entersFromBottom
                    ? 0
                    : Math.max(osd.anchoredFromPanel
                               ? Math.round(placement.y) : 0,
                               Math.max(0, osd.attachmentStartY)))
                   + card.index * osd.stackPeek
                width: osd.cardWidth
                height: osd.cardHeight
                // The file's order, newest in front — and a hovered card
                // rises above the whole file, because a card someone reaches
                // for is the one they are reading.
                z: hoverProbe.hovered ? cardModel.count + 1
                                      : cardModel.count - card.index
                ink: backdropInk
                reducedMotion: osd.reducedMotion
                compositorBlurAvailable: osd.compositorBlurAvailable
                // Only the front card grips the bar: the membrane is one drop
                // out of the icon of the reading it shows.
                attachedToTop: osd.anchoredFromPanel && card.index === 0
                openerRect: osd.openerRect
                attachmentAnchorRect: osd.attachmentAnchorRect
                attachmentStartY: osd.attachmentStartY
                surfacePosition: Qt.point(osd.surfaceOriginX + card.x, card.y)
                // It reports state and cannot be acted on, so it is neither a
                // dialog nor a button to assistive technology.
                Accessible.role: Accessible.StaticText
                Accessible.name: card.cardSpokenText
                // The value changes while the card stays up, so the
                // announcement has to follow it rather than being read once
                // at creation.
                Accessible.description: card.cardSpokenText

                // Leaving is moving away: the card shrinks toward its own
                // centre and fades, and the row is removed only after this
                // has had its beat.
                opacity: card.departing ? 0 : 1
                scale: card.departing ? 0.88 : 1
                transformOrigin: Item.Center
                Behavior on opacity {
                    enabled: !osd.reducedMotion
                    NumberAnimation {
                        duration: CelestinaTheme.motionNormal
                        easing.type: CelestinaTheme.easeExit
                    }
                }
                Behavior on scale {
                    enabled: !osd.reducedMotion
                    NumberAnimation {
                        duration: CelestinaTheme.motionNormal
                        easing.type: CelestinaTheme.easeExit
                    }
                }
                // The receding card keeps its blur to the last frame: the
                // region follows the shrinking mapped bounds — except on the
                // bottom window, whose region is static anyway.
                onScaleChanged: {
                    if (!osd.entersFromBottom)
                        osd.collectGlass();
                }

                // The bottom entry: out of the screen's edge at speed, braking
                // into place, no recoil — an arrival rather than a landing.
                transform: Translate {
                    id: bottomRide

                    y: 0
                }

                NumberAnimation {
                    id: bottomEntry

                    target: bottomRide
                    property: "y"
                    from: osd.cardHeight + CelestinaTheme.spaceLg
                    to: 0
                    duration: CelestinaTheme.motionNormal
                    easing.type: CelestinaTheme.easeStandard
                }

                Component.onCompleted: {
                    card.reveal();
                    if (osd.entersFromBottom && !osd.reducedMotion)
                        bottomEntry.start();
                }
                onGlassRegionsChanged: osd.collectGlass()

                HoverHandler {
                    id: hoverProbe
                }

                // One dense section on the veil, the same anatomy `SoftCard`
                // gives a menu's body: the veil is the carrier and the
                // material a reading sits on is the section, never the
                // carrier turned opaque.
                Item {
                    id: section

                    anchors.fill: parent
                    anchors.margins: CelestinaTheme.spaceMd

                    MenuSection {
                        ink: backdropInk
                    }

                    Column {
                        anchors.fill: parent
                        anchors.margins: CelestinaTheme.spaceMd
                        spacing: CelestinaTheme.spaceSm

                        Row {
                            width: parent.width
                            spacing: CelestinaTheme.spaceSm

                            CelestinaIcon {
                                id: kindIcon

                                anchors.verticalCenter: titleText.verticalCenter
                                width: CelestinaTheme.iconSm
                                height: width
                                name: osd.iconFor(card.kind, card.muted)
                                fallbackName: osd.iconFor(card.kind, card.muted)
                                tintOverride: card.muted ? backdropInk.muted
                                                         : backdropInk.primary
                                Accessible.ignored: true
                            }

                            Text {
                                id: titleText

                                width: parent.width - kindIcon.width
                                       - valueLabel.implicitWidth
                                       - parent.spacing * 2
                                text: card.cardHeadline
                                color: backdropInk.primary
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontBody
                                font.weight: CelestinaTheme.weightDemiBold
                                elide: Text.ElideRight
                            }

                            Text {
                                id: valueLabel

                                anchors.verticalCenter: titleText.verticalCenter
                                text: card.cardValueText
                                // A silenced device keeps the level it
                                // remembers, and the reading says it is not
                                // being heard rather than pretending it moved.
                                color: card.muted ? backdropInk.muted
                                                  : backdropInk.primary
                                font.family: CelestinaTheme.sansFamily
                                font.features: CelestinaTheme.fontFeaturesTabular
                                font.pixelSize: CelestinaTheme.fontBody
                            }
                        }

                        // A meter, not a control: there is nothing to drag
                        // here, and a slider would offer an interaction this
                        // surface cannot accept.
                        Rectangle {
                            id: track

                            width: parent.width
                            height: CelestinaTheme.spaceXs
                            radius: CelestinaTheme.radiusPill
                            visible: card.cardHasLevel
                            // The same track the system slider draws, so a
                            // level reads the same whether it is being shown
                            // or being set.
                            color: backdropInk.divider

                            Rectangle {
                                id: fill

                                height: parent.height
                                radius: parent.radius
                                width: parent.width * Math.max(0, Math.min(1, card.percent / 100))
                                color: card.muted ? backdropInk.muted
                                                  : backdropInk.accent

                                // The level moving is the whole point of the
                                // display, so reduced motion keeps the value
                                // and drops the travel rather than the other
                                // way round.
                                Behavior on width {
                                    enabled: !CelestinaTheme.reducedMotion
                                    NumberAnimation {
                                        duration: CelestinaTheme.motionFast
                                        easing.type: CelestinaTheme.easeStandard
                                    }
                                }
                            }
                        }

                        Text {
                            width: parent.width
                            visible: !card.cardHasLevel && !card.muted
                            text: qsTr("El proveedor no informó de ningún nivel para este dispositivo.")
                            color: backdropInk.muted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontCaption
                            wrapMode: Text.WordWrap
                        }
                    }
                }
            }
        }
            }
        }
    }
}
