// Selector for compositor-native minimized windows supplied by Melibea.
//
// Action acceptance changes no row. Only a later subscribed provider frame
// may remove one, because Niri rather than this surface owns window lifetime.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window
import "ProviderReading.js" as ProviderReading

Window {
    id: overlay

    required property var providerSource
    required property bool reducedMotion
    property alias anchoredFromPanel: placement.anchoredFromPanel
    property alias openerRect: placement.openerRect
    property alias attachmentAnchorRect: placement.attachmentAnchorRect
    property alias attachmentStartY: placement.attachmentStartY
    // M7 — where this output's bubbles sit, handed over by the controller that built this
    // surface. A snapshot is honest here: this overlay is anchored to that panel for its
    // whole short life, so a panel that moved would already have misplaced the card.
    // Absent when the shell has no usable anchor, which asks Niri for its ordinary motion.
    property rect bubbleAnchorRect: Qt.rect(0, 0, 0, 0)
    property string bubbleAnchorOutput: ""
    property alias compositorBlurAvailable: card.compositorBlurAvailable
    property alias glassRects: card.glassRects
    property alias glassRegions: card.glassRegions

    signal dismissed()

    readonly property int cardWidth: 460
    readonly property int cardHeight: 360
    readonly property int anchorGap: placement.anchorGap
    readonly property real cardX: placement.x
    readonly property real cardY: placement.y
    readonly property var bubbleState:
        ProviderReading.read(overlay.providerSource, "melibea")
    readonly property var windows: bubbleState !== undefined
                                   && bubbleState.available === true
                                   && bubbleState.windows !== undefined
                                   ? bubbleState.windows : []
    readonly property int rowCount: windows.length

    property int currentIndex: rowCount > 0 ? 0 : -1
    property double pendingRequest: 0
    property string pendingWindowId: ""
    property string pendingVerb: ""
    property string errorText: ""
    property real shellScale: 1.0
    readonly property real surfaceWidth: width / shellScale
    readonly property real surfaceHeight: height / shellScale

    width: Math.round(cardWidth * shellScale)
    height: Math.round(cardHeight * shellScale)
    color: CelestinaTheme.clear
    title: qsTr("Burbujas de aplicaciones")

    BackdropInk { id: backdropInk }

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = reducedMotion;
        bubbleList.forceActiveFocus();
    }

    PanelPopupPlacement {
        id: placement

        surfaceWidth: overlay.surfaceWidth
        surfaceHeight: overlay.surfaceHeight
        contentWidth: overlay.cardWidth
        contentHeight: overlay.cardHeight
        edgeInset: 0
    }

    function indexOfWindow(windowId) {
        for (let index = 0; index < windows.length; ++index) {
            if (String(windows[index].id) === String(windowId))
                return index;
        }
        return -1;
    }

    // The presentation hint for one action, built now rather than stored.
    //
    // Reduced motion wins over the anchor: someone who asked for less movement is asking
    // for no travel at all, not for travel somewhere else. A close carries no hint because
    // it has no destination to travel to.
    function transitionOptions(verb) {
        if (verb === "close")
            return {};
        if (overlay.reducedMotion)
            return {"transition": "disabled"};
        if (overlay.bubbleAnchorOutput.length === 0
            || overlay.bubbleAnchorRect.width <= 0
            || overlay.bubbleAnchorRect.height <= 0)
            return {};
        return {
            "transition": "anchored",
            "anchor_output": overlay.bubbleAnchorOutput,
            "anchor_x": overlay.bubbleAnchorRect.x,
            "anchor_y": overlay.bubbleAnchorRect.y,
            "anchor_width": overlay.bubbleAnchorRect.width,
            "anchor_height": overlay.bubbleAnchorRect.height
        };
    }

    function sendAction(verb, windowId) {
        if (!providerSource || pendingRequest !== 0)
            return;
        errorText = "";
        pendingWindowId = String(windowId);
        pendingVerb = verb;
        const options = overlay.transitionOptions(verb);
        options["window_id"] = pendingWindowId;
        pendingRequest = providerSource.sendCommand("melibea", verb, options);
        if (pendingRequest === 0) {
            pendingWindowId = "";
            pendingVerb = "";
            errorText = qsTr("Melibea no está disponible");
        }
    }

    function restoreCurrent() {
        if (currentIndex >= 0 && currentIndex < windows.length)
            sendAction("restore", windows[currentIndex].id);
    }

    function closeCurrent() {
        if (currentIndex >= 0 && currentIndex < windows.length)
            sendAction("close", windows[currentIndex].id);
    }

    function reconcileWindows() {
        let completedVerb = "";
        if (pendingWindowId.length > 0
            && indexOfWindow(pendingWindowId) < 0) {
            completedVerb = pendingVerb;
            pendingRequest = 0;
            pendingWindowId = "";
            pendingVerb = "";
        }
        if (rowCount === 0) {
            currentIndex = -1;
            dismissed();
        } else if (completedVerb === "restore") {
            // Niri focuses the restored surface. Retire this chooser once the
            // subscribed state confirms that hand-off instead of leaving an
            // unfocused card over the window the person just recovered.
            dismissed();
        } else if (currentIndex < 0) {
            currentIndex = 0;
        } else if (currentIndex >= rowCount) {
            currentIndex = rowCount - 1;
        }
    }

    // The authoritative list can replace one window with another without
    // changing its length. Reconcile against window identity on every frame,
    // not only against the count, or such a replacement would leave the
    // selector permanently pending.
    onWindowsChanged: reconcileWindows()

    Connections {
        target: overlay.providerSource

        function onCommandResult(requestId, state, reason) {
            if (requestId !== overlay.pendingRequest)
                return;
            if (state === "failed") {
                overlay.pendingRequest = 0;
                overlay.pendingWindowId = "";
                overlay.pendingVerb = "";
                overlay.errorText = qsTr("No se pudo cambiar el estado de la ventana");
            }
            // `accepted` and `confirmed` do not mutate the visual model. The
            // subscribed `melibea.windows` reading is the only confirmation
            // that may remove a row.
        }
    }

    Shortcut {
        sequence: "Escape"
        context: Qt.WindowShortcut
        onActivated: overlay.dismissed()
    }

    Shortcut {
        sequence: "Delete"
        context: Qt.WindowShortcut
        enabled: overlay.rowCount > 0 && overlay.pendingRequest === 0
        onActivated: overlay.closeCurrent()
    }

    Item {
        id: shellScene
        objectName: "celestina-shell-scene"
        width: overlay.surfaceWidth
        height: overlay.surfaceHeight
        transformOrigin: Item.TopLeft
        scale: overlay.shellScale
    }

    MouseArea {
        parent: shellScene
        z: -1
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        onPressed: overlay.dismissed()
    }

    Item {
        parent: shellScene
        width: overlay.cardWidth
        height: overlay.cardHeight
        x: overlay.cardX
        y: overlay.cardY

        SoftOverlayCard {
            id: card
            anchors.fill: parent
            ink: backdropInk
            reducedMotion: overlay.reducedMotion
            accessibleName: qsTr("Ventanas minimizadas")
            attachedToTop: overlay.anchoredFromPanel
            openerRect: overlay.openerRect
            attachmentAnchorRect: overlay.attachmentAnchorRect
            attachmentStartY: overlay.attachmentStartY
            surfacePosition: Qt.point(overlay.cardX, overlay.cardY)

            Item {
                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceMd

                MenuHeader {
                    id: header
                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    ink: backdropInk
                    title: qsTr("Burbujas")
                    subtitle: qsTr("%n ventana(s) apartada(s)", "", overlay.rowCount)
                    iconName: "app-window"
                }

                Item {
                    anchors.top: header.bottom
                    anchors.topMargin: CelestinaTheme.spaceSm
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom

                    MenuSection { ink: backdropInk }

                    Column {
                        anchors.fill: parent
                        anchors.margins: CelestinaTheme.spaceSm
                        spacing: CelestinaTheme.spaceXs

                        Text {
                            width: parent.width
                            visible: overlay.errorText.length > 0
                            text: overlay.errorText
                            color: backdropInk.danger
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontCaption
                            wrapMode: Text.Wrap
                        }

                        ListView {
                            id: bubbleList
                            objectName: "celestina-bubble-list"
                            width: parent.width
                            height: parent.height
                                    - (overlay.errorText.length > 0
                                       ? parent.spacing + 28 : 0)
                            clip: true
                            spacing: 2
                            model: overlay.windows
                            currentIndex: overlay.currentIndex
                            activeFocusOnTab: true
                            Accessible.role: Accessible.List
                            Accessible.name: qsTr("Ventanas minimizadas")
                            onCurrentIndexChanged:
                                overlay.currentIndex = currentIndex
                            Keys.onPressed: function(event) {
                                if (event.key === Qt.Key_Down && overlay.rowCount > 0) {
                                    overlay.currentIndex = Math.min(
                                        overlay.rowCount - 1,
                                        overlay.currentIndex + 1);
                                } else if (event.key === Qt.Key_Up
                                           && overlay.rowCount > 0) {
                                    overlay.currentIndex = Math.max(
                                        0, overlay.currentIndex - 1);
                                } else if (event.key === Qt.Key_Return
                                           || event.key === Qt.Key_Enter) {
                                    overlay.restoreCurrent();
                                } else {
                                    return;
                                }
                                event.accepted = true;
                            }

                            delegate: Item {
                                id: row
                                required property int index
                                required property var modelData
                                objectName: "celestina-bubble-row-" + index

                                readonly property bool current:
                                    overlay.currentIndex === index
                                readonly property string windowId:
                                    String(modelData.id)
                                readonly property string appIdentity:
                                    modelData.iconName !== undefined
                                    && modelData.iconName.length > 0
                                    ? modelData.iconName
                                    : modelData.appId !== undefined
                                      ? modelData.appId : ""
                                readonly property string displayTitle:
                                    modelData.title !== undefined
                                    && modelData.title.length > 0
                                    ? modelData.title
                                    : modelData.appId !== undefined
                                      && modelData.appId.length > 0
                                      ? modelData.appId
                                      : qsTr("Ventana sin título")
                                readonly property bool pending:
                                    overlay.pendingWindowId === windowId

                                width: ListView.view.width
                                height: 54
                                Accessible.role: Accessible.ListItem
                                Accessible.name: displayTitle
                                Accessible.description: qsTr("Restaurar ventana")
                                Accessible.selected: current
                                Accessible.onPressAction: {
                                    overlay.currentIndex = index;
                                    overlay.restoreCurrent();
                                }

                                Rectangle {
                                    anchors.fill: parent
                                    radius: CelestinaTheme.radiusSm
                                    color: restoreArea.pressed
                                           ? backdropInk.selectedFill
                                           : row.current
                                             ? backdropInk.selectedRestFill
                                             : restoreArea.containsMouse
                                               ? backdropInk.hoverFill
                                               : CelestinaTheme.clear
                                }

                                Rectangle {
                                    id: iconBubble
                                    anchors.left: parent.left
                                    anchors.leftMargin: CelestinaTheme.spaceSm
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 34
                                    height: width
                                    radius: width / 2
                                    color: backdropInk.controlFill

                                    Image {
                                        id: applicationIcon
                                        anchors.centerIn: parent
                                        width: 24
                                        height: width
                                        sourceSize: Qt.size(32, 32)
                                        fillMode: Image.PreserveAspectFit
                                        visible: status === Image.Ready
                                        source: row.appIdentity.length > 0
                                                ? "image://appicon/"
                                                  + encodeURIComponent(
                                                      row.appIdentity)
                                                : ""
                                    }

                                    CelestinaIcon {
                                        anchors.centerIn: parent
                                        width: 22
                                        height: width
                                        visible: !applicationIcon.visible
                                        name: "app-window"
                                        fallbackName: "app-window"
                                        tintOverride: backdropInk.primary
                                        Accessible.ignored: true
                                    }
                                }

                                Column {
                                    anchors.left: iconBubble.right
                                    anchors.leftMargin: CelestinaTheme.spaceSm
                                    anchors.right: closeButton.left
                                    anchors.rightMargin: CelestinaTheme.spaceSm
                                    anchors.verticalCenter: parent.verticalCenter

                                    Text {
                                        objectName: "celestina-bubble-title-"
                                                    + row.index
                                        width: parent.width
                                        text: row.displayTitle
                                        textFormat: Text.PlainText
                                        color: row.current ? backdropInk.accent
                                                           : backdropInk.primary
                                        font.family: CelestinaTheme.sansFamily
                                        font.pixelSize: CelestinaTheme.fontRowSecondary
                                        elide: Text.ElideRight
                                    }

                                    Text {
                                        width: parent.width
                                        visible: text.length > 0
                                        text: row.pending
                                              ? qsTr("Esperando al compositor…")
                                              : row.modelData.appId !== undefined
                                                ? row.modelData.appId : ""
                                        textFormat: Text.PlainText
                                        color: backdropInk.muted
                                        font.family: CelestinaTheme.sansFamily
                                        font.pixelSize: CelestinaTheme.fontMini
                                        elide: Text.ElideRight
                                    }
                                }

                                BackdropIconButton {
                                    id: closeButton
                                    objectName: "celestina-bubble-close-"
                                                + row.index
                                    anchors.right: parent.right
                                    anchors.rightMargin: CelestinaTheme.spaceXs
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 34
                                    height: width
                                    ink: backdropInk
                                    iconName: "x"
                                    fallbackIcon: "x"
                                    helpText: qsTr("Cerrar ventana")
                                    role: CelestinaButton.Destructive
                                    enabled: overlay.pendingRequest === 0
                                    onClicked: {
                                        overlay.currentIndex = row.index;
                                        overlay.closeCurrent();
                                    }
                                }

                                MouseArea {
                                    id: restoreArea
                                    objectName: "celestina-bubble-restore-"
                                                + row.index
                                    anchors.left: parent.left
                                    anchors.top: parent.top
                                    anchors.bottom: parent.bottom
                                    anchors.right: closeButton.left
                                    hoverEnabled: true
                                    enabled: overlay.pendingRequest === 0
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        overlay.currentIndex = row.index;
                                        overlay.restoreCurrent();
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
