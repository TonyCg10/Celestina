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
        for (let index = 0; index < column.children.length; ++index) {
            const card = column.children[index];
            if (!card || card.glassRegions === undefined)
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
                                               : column.implicitHeight
        contentWidth: stack.cardWidth
        contentHeight: column.implicitHeight
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

    // The window is as tall as what it holds: attached, that is the seam, the
    // connector and the whole column; floating, the column alone. The stack
    // grows as toasts arrive and the compositor is asked to follow.
    readonly property real neededHeight: stack.anchoredFromPanel
            ? Math.max(stack.surfaceHeight,
                       placement.y + column.implicitHeight
                       + CelestinaTheme.spaceLg)
            : column.implicitHeight

    width: Math.round(stack.surfaceWidth * stack.shellScale)
    height: Math.round(stack.neededHeight * stack.shellScale)
    color: CelestinaTheme.clear
    title: qsTr("Notificaciones de Celestina")

    Component.onCompleted: CelestinaTheme.reducedMotion = stack.reducedMotion

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

    // A dismissed toast destroys its delegate, and a destroyed field emits
    // nothing — without this the union kept the dead card's region and the
    // compositor kept blurring where it had been.
    onToastsChanged: Qt.callLater(stack.collectGlass)

    // A card collects its region on a timer after its reveal; a window that
    // was mapped later than that would otherwise hold regions collected while
    // nothing was on screen. Asking again on the mapping costs one walk.
    onVisibleChanged: {
        if (!stack.visible)
            return;
        for (let index = 0; index < column.children.length; ++index) {
            const card = column.children[index];
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

    Column {
        id: column

        x: Math.round(placement.x)
        y: stack.anchoredFromPanel ? Math.round(placement.y) : 0
        width: stack.cardWidth
        spacing: stack.cardSpacing

        Repeater {
            model: stack.toasts

            delegate: SoftMenuField {
                id: card

                required property var modelData
                required property int index

                readonly property bool critical: card.modelData.urgency === "critical"
                readonly property var offered: stack.actionsFor(card.modelData.id)
                // Chained because QML's `arg` substitutes one value per call,
                // unlike its C++ namesake. A producer that puts a `%2` in its
                // own app name can therefore consume the next substitution and
                // garble this sentence; it cannot reach past it, and the
                // rendered text is inert either way.
                readonly property string spokenText: qsTr("%1: %2. %3")
                    .arg(card.modelData.app)
                    .arg(card.modelData.summary)
                    .arg(card.modelData.body)

                width: stack.cardWidth
                // Measured, never estimated: the laid-out body plus the veil's
                // margin and the section's own padding, which is the same
                // anatomy `SoftCard` gives a menu.
                // Implicit, not merely set: the column sums implicit heights,
                // and a card that stated only `height` summed to a zero-tall
                // column — a fallback window nobody could see.
                implicitHeight: body.implicitHeight + CelestinaTheme.spaceMd * 4
                height: implicitHeight
                ink: backdropInk
                reducedMotion: stack.reducedMotion
                compositorBlurAvailable: stack.compositorBlurAvailable
                // Only the first card grips the bar: the membrane is one drop
                // out of the bell, and the rest of the column hangs from it.
                attachedToTop: stack.anchoredFromPanel && card.index === 0
                openerRect: stack.openerRect
                attachmentAnchorRect: stack.attachmentAnchorRect
                attachmentStartY: stack.attachmentStartY
                surfacePosition: Qt.point(stack.surfaceOriginX + column.x,
                                          column.y + card.y)
                Accessible.role: Accessible.Notification
                Accessible.name: card.spokenText

                // Arriving is worth a movement, and it is the field's own
                // reveal. A stack that reflows because the toast above it left
                // republishes from the field's geometry change, not from here.
                Component.onCompleted: card.reveal()
                onGlassRegionsChanged: stack.collectGlass()

                Item {
                    id: section

                    anchors.fill: parent
                    anchors.margins: CelestinaTheme.spaceMd

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
                                text: card.modelData.app
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
                                onClicked: stack.dismiss(card.modelData.id)
                            }
                        }

                        Text {
                            width: parent.width
                            text: card.modelData.summary
                            textFormat: Text.PlainText
                            color: backdropInk.primary
                            elide: Text.ElideRight
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontBody
                            font.weight: CelestinaTheme.weightDemiBold
                        }

                        Text {
                            width: parent.width
                            visible: card.modelData.body.length > 0
                            text: card.modelData.body
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
                                    onClicked: stack.invoke(card.modelData.id, modelData.key)
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
