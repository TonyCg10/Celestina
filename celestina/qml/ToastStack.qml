// The corner where this session's notifications appear.
//
// A toast is a glance, not a workspace: it shows who is talking, what they
// said and the buttons they offered, and it leaves when the server says it has.
// Nothing here decides how long that is — the shell's notification server owns
// every rule about expiry, replacement and caps, and this surface paints the
// list it publishes.
//
// It never takes the keyboard, so its buttons are reachable by pointer here and
// by keyboard in the notification centre. That is the deliberate split: a
// surface that grabbed focus every time an application spoke would interrupt
// typing, which is the one thing a notification must not do.
//
// PANEL-1. Each toast is made of the shell's glass, like the bar's readings,
// its menus and the on-screen display: one `SoftMenuField` veil carrying one
// denser `ContentSurface` section. It used to be a `GlassCard`, which takes
// its material from an in-scene capture — and this window's scene holds
// nothing behind the cards, so every toast fell back to an opaque tint and
// read as a solid plate over the desktop. The stack is one surface with
// several cards on it, so it publishes the union of their glass regions
// rather than one region of its own.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: stack

    // Each entry carries `id`, `app`, `summary`, `body`, `urgency`, `read` and
    // an `actions` list of `{key, label}`. `var` is necessary: QML has no typed
    // map-list.
    required property var toasts
    // Every action of every toast, each naming the notification it belongs to.
    // They arrive beside the toasts rather than inside them: the host accepts
    // one level of structure, and a row carrying its own list took the whole
    // frame — and with it the rest of the bar — down on a live session.
    required property var actions
    required property var providerSource
    required property bool reducedMotion

    // The compositor sample the cards draw over. `OverlaySurface` arms a
    // `PanelBlurController` for any window publishing these, and the
    // controller writes `compositorBlurAvailable` back when the effect took.
    property bool compositorBlurAvailable: false
    property var glassRects: []
    property var glassRegions: []

    // One surface, several cards: the union of what each card collected, in
    // this window's coordinates — which is what every field already reports.
    // A card that is still measuring contributes nothing rather than a region
    // at the wrong place.
    function collectGlass() {
        const rects = [];
        const regions = [];
        for (let index = 0; index < scene.children.length; ++index) {
            const card = scene.children[index];
            // A hidden field keeps its last published regions as data; what
            // the compositor must see is that nothing visible asks for glass.
            if (!card || !card.visible || card.glassRegions === undefined)
                continue;
            for (let each = 0; each < card.glassRegions.length; ++each)
                regions.push(card.glassRegions[each]);
            for (let rect = 0; rect < card.glassRects.length; ++rect)
                rects.push(card.glassRects[rect]);
        }
        stack.glassRects = rects;
        stack.glassRegions = regions;
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

        surfaceWidth: stack.surfaceWidth
        // The host's stated height, never the growing one: the placement's own
        // clamp reads this, and a value that followed the column's growth
        // would read the y it is being asked to produce — the binding loop.
        // The connector's y sits far above either value, so the clamp is
        // unaffected.
        surfaceHeight: stack.anchoredFromPanel ? stack.surfaceHeight
                                               : field.targetHeight
        contentWidth: stack.cardWidth
        contentHeight: field.targetHeight
        anchoredFromPanel: stack.anchoredFromPanel
        openerRect: Qt.rect(stack.openerRect.x - stack.surfaceOriginX,
                            stack.openerRect.y,
                            stack.openerRect.width, stack.openerRect.height)
        attachmentAnchorRect: stack.attachmentAnchorRect
        attachmentStartY: stack.attachmentStartY
        fallbackX: 0
        fallbackY: 0
        edgeInset: stack.anchoredFromPanel ? CelestinaTheme.spaceSm : 0
    }

    readonly property int cardWidth: 380
    readonly property int cardSpacing: CelestinaTheme.spaceSm

    // The stack appears attached to the bar, a drop out of the panel's own
    // notification bell — the same contract every menu carries, resolved by
    // the host because a toast arrives without a click. All rectangles are
    // output-local shell units. Left at the defaults the stack is the
    // floating column it has always been, which is also the fallback the
    // host uses when the top-right zone is already taken.
    property bool anchoredFromPanel: false
    property rect openerRect: Qt.rect(0, 0, 0, 0)
    property rect attachmentAnchorRect: Qt.rect(0, 0, 0, 0)
    property real attachmentStartY: -1
    property real surfaceOriginX: 0
    property real surfaceWidth: stack.cardWidth
    property real surfaceHeight: 0
    // How much larger this output needs the shell drawn; see shellscale.h.
    property real shellScale: 1.0
    // The bottom-centre fallback sits flush with the screen's bottom edge and
    // behaves like the display's own fallback: the block enters by physically
    // emerging from the edge, leaves by receding into the distance, and the
    // pile grows upward — newest at the edge, the rest riding above it.
    property bool entersFromBottom: false

    // The window is as tall as what it holds: attached, that is the seam, the
    // connector and the whole column; floating, the column alone. The stack
    // grows as toasts arrive and the compositor is asked to follow.
    readonly property real neededHeight: stack.anchoredFromPanel
            ? Math.max(stack.surfaceHeight,
                       placement.y + field.targetHeight
                       + CelestinaTheme.spaceLg)
            : field.targetHeight
              + (stack.entersFromBottom ? CelestinaTheme.spaceLg : 0)

    width: Math.round(stack.surfaceWidth * stack.shellScale)
    height: Math.round(stack.neededHeight * stack.shellScale)
    color: CelestinaTheme.clear
    title: qsTr("Notificaciones de Celestina")

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = stack.reducedMotion;
        stack.syncToasts();
    }

    // The actions offered by one notification.
    function actionsFor(id) {
        const mine = [];
        for (let index = 0; index < stack.actions.length; ++index) {
            if (stack.actions[index].notification === id)
                mine.push(stack.actions[index]);
        }
        return mine;
    }

    function dismiss(id) {
        if (stack.providerSource)
            stack.providerSource.sendCommand("notifications", "dismiss", {"id": id});
    }

    function invoke(id, key) {
        if (stack.providerSource) {
            stack.providerSource.sendCommand("notifications", "invoke",
                                             {"id": id, "action": key});
        }
    }

    // The model is kept in place rather than rebuilt: a plain list handed to
    // the Repeater recreated every delegate on every arrival, which replayed
    // every card's reveal each time one toast was added — the whole column
    // dripping again because one message came in. Rows are keyed by the
    // server's own notification id.
    ListModel {
        id: toastModel
    }

    // The sweep that finally clears a receded block, once its exit has had
    // its full beat on screen.
    Timer {
        id: blockDeparture

        interval: CelestinaTheme.motionNormal
        onTriggered: {
            toastModel.clear();
            field.departing = false;
            Qt.callLater(stack.collectGlass);
        }
    }

    // The sweep that removes rows whose collapse has had its full beat: a
    // toast that leaves while others stay is covered rather than cut — its
    // section fades and folds shut underneath while the survivors slide over
    // its place, on both routes.
    Timer {
        id: rowSweep

        interval: CelestinaTheme.motionNormal
        onTriggered: {
            for (let stale = toastModel.count - 1; stale >= 0; --stale) {
                if (toastModel.get(stale).leaving === true)
                    toastModel.remove(stale);
            }
            Qt.callLater(stack.collectGlass);
        }
    }

    function syncToasts() {
        const list = stack.toasts;
        const wasEmpty = toastModel.count === 0;

        // On the bottom route an emptied list plays the block's recede — the
        // display's own moving-away — and is swept after; a toast that
        // arrives mid-recede reclaims the block. This is the last section's
        // exit; a section with survivors is covered instead, below.
        if (stack.entersFromBottom && !stack.reducedMotion) {
            if (list.length === 0 && toastModel.count > 0) {
                if (!field.departing) {
                    field.departing = true;
                    blockDeparture.restart();
                }
                return;
            }
            if (list.length > 0 && field.departing) {
                blockDeparture.stop();
                field.departing = false;
            }
        }

        // The bottom pile mirrors the top one: new sections are born away
        // from the screen's edge — upward — so the oldest sits at the edge
        // and the survivors slide down over it when it leaves.
        const desired = stack.entersFromBottom
            ? list.slice().reverse() : list.slice();

        for (let stale = toastModel.count - 1; stale >= 0; --stale) {
            let present = false;
            for (let index = 0; index < desired.length; ++index) {
                if (desired[index].id === toastModel.get(stale).noteId)
                    present = true;
            }
            if (present)
                continue;
            if (stack.reducedMotion || desired.length === 0) {
                toastModel.remove(stale);
            } else if (toastModel.get(stale).leaving !== true) {
                toastModel.setProperty(stale, "leaving", true);
                rowSweep.restart();
            }
        }
        for (let index = 0; index < desired.length; ++index) {
            const entry = {
                "noteId": desired[index].id,
                "app": desired[index].app !== undefined ? desired[index].app : "",
                "summary": desired[index].summary !== undefined
                           ? desired[index].summary : "",
                "body": desired[index].body !== undefined ? desired[index].body : "",
                "urgency": desired[index].urgency !== undefined
                           ? desired[index].urgency : "",
                "leaving": false
            };
            let at = -1;
            for (let have = 0; have < toastModel.count; ++have) {
                if (toastModel.get(have).noteId === entry.noteId)
                    at = have;
            }
            if (at < 0) {
                toastModel.insert(Math.min(index, toastModel.count), entry);
            } else {
                toastModel.set(at, entry);
                if (at !== index && index < toastModel.count)
                    toastModel.move(at, index, 1);
            }
        }
        // The first notification is what creates the block: the field is
        // already alive on the persistent window, so its arrival replays for
        // each fresh first — the drop out of the bell up top, the emergence
        // from the screen's edge down below.
        if (wasEmpty && toastModel.count > 0) {
            Qt.callLater(field.reveal);
            if (stack.entersFromBottom && !stack.reducedMotion)
                blockEntry.start();
        }
    }

    // A dismissed toast destroys its delegate, and a destroyed field emits
    // nothing — without this the union kept the dead card's region and the
    // compositor kept blurring where it had been.
    onToastsChanged: {
        stack.syncToasts();
        Qt.callLater(stack.collectGlass);
    }

    // A card collects its region on a timer after its reveal; a window that
    // was mapped later than that would otherwise hold regions collected while
    // nothing was on screen. Asking again on the mapping costs one walk.
    onVisibleChanged: {
        if (!stack.visible)
            return;
        for (let index = 0; index < scene.children.length; ++index) {
            const card = scene.children[index];
            if (card && card.scheduleGlassCollection !== undefined)
                card.scheduleGlassCollection();
        }
    }

    // Every card lives inside a scene item that carries the per-output
    // factor, exactly as the panel's own scene does.
    Item {
        id: scene

        width: stack.surfaceWidth
        height: stack.neededHeight
        transformOrigin: Item.TopLeft
        scale: stack.shellScale

    // One block of glass, not a pile of blocks: the author's direction is
    // that the first notification creates the field and every later one
    // expands that same field. The veil is this single carrier — the one
    // membrane out of the bell — and each notification is a denser section
    // inside it, exactly the anatomy a menu gives its rows.
    SoftMenuField {
        id: field

        x: Math.round(placement.x)
        // Attached, under the seam; flush at the bottom, pinned by its lower
        // edge — the breathing room stays between the block and the screen's
        // edge, and growth moves the top edge, never the bottom.
        // The top routes are clamped at the seam whatever the routing knows
        // so far: before `anchoredFromPanel` settles this read 0, which on
        // the attached window is the screen's top edge, over the bar.
        y: stack.entersFromBottom
           ? stack.neededHeight - CelestinaTheme.spaceLg - field.height
           : Math.max(stack.anchoredFromPanel ? Math.round(placement.y) : 0,
                      Math.max(0, stack.attachmentStartY))
        width: stack.cardWidth
        // As tall as everything it holds; the growth is a movement of its
        // own, synchronized with the arriving section's slide by sharing its
        // duration and curve.
        readonly property real targetHeight:
            rows.implicitHeight + CelestinaTheme.spaceMd * 2
        height: field.targetHeight
        Behavior on height {
            enabled: !stack.reducedMotion
            NumberAnimation {
                duration: CelestinaTheme.motionNormal
                easing.type: CelestinaTheme.easeStandard
            }
        }
        // The compositor region follows the growing edge per frame, as every
        // falling route already does: a field whose blur waited at the final
        // size would show the expansion over a bare backdrop.
        onHeightChanged: field.collectGlass()
        visible: toastModel.count > 0

        // Leaving, on the bottom route, is moving away — the same recede the
        // display's card plays: shrinking toward its centre and fading, with
        // the rows kept alive for the beat and swept only after it. The
        // region stays where it was published for those frames, exactly as
        // the display's own bottom window keeps its static footprint.
        property bool departing: false
        opacity: field.departing ? 0 : 1
        scale: field.departing ? 0.88 : 1
        transformOrigin: Item.Center
        Behavior on opacity {
            enabled: !stack.reducedMotion && stack.entersFromBottom
            NumberAnimation {
                duration: CelestinaTheme.motionNormal
                easing.type: CelestinaTheme.easeExit
            }
        }
        Behavior on scale {
            enabled: !stack.reducedMotion && stack.entersFromBottom
            NumberAnimation {
                duration: CelestinaTheme.motionNormal
                easing.type: CelestinaTheme.easeExit
            }
        }

        // The bottom entry: the whole block out of the screen's edge at
        // speed, braking into place, no recoil — the display's own arrival.
        transform: Translate {
            id: blockRide

            y: 0
        }

        NumberAnimation {
            id: blockEntry

            target: blockRide
            property: "y"
            from: field.height + CelestinaTheme.spaceLg
            to: 0
            duration: CelestinaTheme.motionNormal
            easing.type: CelestinaTheme.easeStandard
        }
        ink: backdropInk
        reducedMotion: stack.reducedMotion
        compositorBlurAvailable: stack.compositorBlurAvailable
        attachedToTop: stack.anchoredFromPanel
        openerRect: stack.openerRect
        attachmentAnchorRect: stack.attachmentAnchorRect
        attachmentStartY: stack.attachmentStartY
        surfacePosition: Qt.point(stack.surfaceOriginX + field.x, field.y)
        onGlassRegionsChanged: stack.collectGlass()
        Component.onCompleted: {
            if (toastModel.count > 0)
                field.reveal();
        }

        Column {
            id: rows

            anchors.left: parent.left
            anchors.right: parent.right
            // Downward from the bar, upward from the edge: bottom-anchored,
            // the newest section sits nearest the screen's edge and the rest
            // of the pile rides above it.
            anchors.top: stack.entersFromBottom ? undefined : parent.top
            anchors.bottom: stack.entersFromBottom ? parent.bottom : undefined
            anchors.margins: CelestinaTheme.spaceMd
            spacing: stack.cardSpacing

        Repeater {
            model: toastModel

            delegate: Item {
                id: card

                required property int index
                required property int noteId
                required property string app
                required property string summary
                required property string body
                required property string urgency
                required property bool leaving

                readonly property bool critical: card.urgency === "critical"
                readonly property var offered: stack.actionsFor(card.noteId)
                // Chained because QML's `arg` substitutes one value per call,
                // unlike its C++ namesake. A producer that puts a `%2` in its
                // own app name can therefore consume the next substitution and
                // garble this sentence; it cannot reach past it, and the
                // rendered text is inert either way.
                readonly property string spokenText: qsTr("%1: %2. %3")
                    .arg(card.app)
                    .arg(card.summary)
                    .arg(card.body)

                width: rows.width
                // Measured, never estimated: the laid-out body plus the
                // section's own padding. Implicit, not merely set: the column
                // sums implicit heights, and a row that stated only `height`
                // summed to a zero-tall column — a window nobody could see.
                implicitHeight: body.implicitHeight + CelestinaTheme.spaceMd * 2
                // Leaving is being covered: the section folds shut and fades
                // underneath while the survivors slide over its place — the
                // column reflows with the fold, frame by frame, so the two
                // movements are one.
                height: card.leaving ? 0 : card.implicitHeight
                opacity: card.leaving ? 0 : 1
                clip: card.leaving
                Behavior on height {
                    enabled: !stack.reducedMotion
                    NumberAnimation {
                        duration: CelestinaTheme.motionNormal
                        easing.type: CelestinaTheme.easeStandard
                    }
                }
                Behavior on opacity {
                    enabled: !stack.reducedMotion
                    NumberAnimation {
                        duration: CelestinaTheme.motionNormal
                        easing.type: CelestinaTheme.easeExit
                    }
                }
                // Age above youth while arriving — the section before must
                // paint on top for the whole ride — and a leaving section
                // drops under everything, because the survivors move over it.
                z: card.leaving ? -toastModel.count - 1
                   : stack.entersFromBottom ? card.index : -card.index
                Accessible.role: Accessible.Notification
                Accessible.name: card.spokenText

                // Emerging from behind its neighbour: downward from behind
                // the one above on the bar route, upward from behind the one
                // below on the edge route — braking, no recoil, while the
                // shared field's edge grows to meet it on the same curve.
                transform: Translate {
                    id: arrivalRide

                    y: 0
                }

                NumberAnimation {
                    id: arrival

                    target: arrivalRide
                    property: "y"
                    from: (stack.entersFromBottom ? 1 : -1)
                          * (card.implicitHeight + stack.cardSpacing)
                    to: 0
                    duration: CelestinaTheme.motionNormal
                    easing.type: CelestinaTheme.easeStandard
                }

                Component.onCompleted: {
                    // A section joining a block that is already up; the first
                    // one rides in with the block itself.
                    if (toastModel.count > 1 && !stack.reducedMotion)
                        arrival.start();
                }

                MenuSection {
                    ink: backdropInk
                }

                // A critical notification is the one case where the surface
                // says so on its own: the server will never time it out, so
                // a person needs to see that it is different.
                Rectangle {
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    anchors.margins: CelestinaTheme.spaceXs
                    width: CelestinaTheme.spaceXs
                    radius: CelestinaTheme.radiusPill
                    visible: card.critical
                    color: CelestinaTheme.danger
                    z: 1
                }

                Column {
                    id: body

                    anchors.fill: parent
                    anchors.margins: CelestinaTheme.spaceMd
                    spacing: CelestinaTheme.spaceXs

                        Row {
                            width: parent.width
                            spacing: CelestinaTheme.spaceSm

                            Text {
                                id: appLabel

                                width: parent.width - dismissButton.width - parent.spacing
                                text: card.app
                                // Whatever a producer sent is shown as the
                                // characters it sent. `AutoText` would guess this
                                // was markup and render it — a link, or an image
                                // this shell would then fetch on the producer's
                                // behalf. The server never advertises `body-markup`,
                                // so honouring markup would be a promise nobody made.
                                textFormat: Text.PlainText
                                color: backdropInk.muted
                                elide: Text.ElideRight
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontCaption
                            }

                            // On glass, ink comes from the surface it sits on.
                            // These were the shell's one direct style-control
                            // exception because the card was an opaque plate of
                            // its own; it is a menu material now, so they join the
                            // BackdropButton family every other card uses and
                            // inherit its no-hover-card contract.
                            BackdropIconButton {
                                id: dismissButton

                                objectName: "celestina-toast-dismiss"
                                width: CelestinaTheme.controlHeightXs
                                height: width
                                ink: backdropInk
                                iconName: "x"
                                // Dismissing is this person having dealt with it,
                                // which is not what a timeout means.
                                helpText: qsTr("Descartar esta notificación")
                                Accessible.name: helpText
                                onClicked: stack.dismiss(card.noteId)
                            }
                        }

                        Text {
                            width: parent.width
                            text: card.summary
                            textFormat: Text.PlainText
                            color: backdropInk.primary
                            elide: Text.ElideRight
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontBody
                            font.weight: CelestinaTheme.weightDemiBold
                        }

                        Text {
                            width: parent.width
                            visible: card.body.length > 0
                            text: card.body
                            textFormat: Text.PlainText
                            color: backdropInk.muted
                            wrapMode: Text.WordWrap
                            maximumLineCount: 3
                            elide: Text.ElideRight
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontCaption
                        }

                        Row {
                            width: parent.width
                            visible: card.offered.length > 0
                            spacing: CelestinaTheme.spaceSm

                            Repeater {
                                model: card.offered

                                delegate: BackdropButton {
                                    required property var modelData

                                    ink: backdropInk
                                    text: modelData.label
                                    onClicked: stack.invoke(card.noteId, modelData.key)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    }
}
