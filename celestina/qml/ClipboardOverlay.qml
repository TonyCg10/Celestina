// The clipboard history: a keyboard-driven list of recent selections, in its
// own compositor surface — the same `OverlayController`/`OverlaySurface`
// mechanics the launcher uses. Every entry is a preview only, never the full
// text (the `clipboard` provider bounds what it publishes for exactly that
// reason); choosing one asks the provider to set it as the selection again by
// its index, never by echoing the preview text back.
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
    property alias compositorBlurAvailable: card.compositorBlurAvailable
    property alias glassRects: card.glassRects
    property alias glassRegions: card.glassRegions

    signal dismissed()

    BackdropInk {
        id: backdropInk
    }

    readonly property int cardWidth: 460
    readonly property int cardHeight: 420
    readonly property int anchorGap: placement.anchorGap
    readonly property real cardX: placement.x
    readonly property real cardY: placement.y

    readonly property var clipboardState: ProviderReading.read(overlay.providerSource, "clipboard")
    readonly property bool offered: clipboardState !== undefined
    readonly property var entries: clipboardState && clipboardState.entries !== undefined
                                   ? clipboardState.entries : []
    readonly property bool truncated: clipboardState !== undefined
                                      && clipboardState.truncated === true

    property int currentIndex: entries.length > 0 ? 0 : -1

    width: cardWidth
    height: cardHeight
    color: CelestinaTheme.clear
    title: qsTr("Historial del portapapeles")

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = reducedMotion;
        overlay.takeFocus();
    }

    onVisibleChanged: {
        if (visible)
            Qt.callLater(card.reveal);
    }

    PanelPopupPlacement {
        id: placement

        surfaceWidth: overlay.width
        surfaceHeight: overlay.height
        contentWidth: overlay.cardWidth
        contentHeight: overlay.cardHeight
        edgeInset: 0
    }

    // The list owns the keyboard while it exists, and it stops existing when
    // the history is emptied — which left `Vaciar` producing an overlay that
    // could not be closed, because the only thing handling Escape had just
    // become invisible. Focus falls back to the card, which is always there.
    function takeFocus() {
        if (overlay.entries.length > 0)
            entryList.forceActiveFocus();
        else
            card.forceActiveFocus();
    }

    onEntriesChanged: {
        overlay.takeFocus();
        if (overlay.currentIndex >= overlay.entries.length)
            overlay.currentIndex = overlay.entries.length > 0 ? overlay.entries.length - 1 : -1;
        else if (overlay.currentIndex < 0 && overlay.entries.length > 0)
            overlay.currentIndex = 0;
    }

    function select(index) {
        if (!overlay.providerSource || index < 0 || index >= overlay.entries.length)
            return;
        overlay.providerSource.sendCommand("clipboard", "select", {"index": index});
        overlay.dismissed();
    }

    function remove(index) {
        if (!overlay.providerSource || index < 0 || index >= overlay.entries.length)
            return;
        overlay.providerSource.sendCommand("clipboard", "remove", {"index": index});
    }

    function clear() {
        if (overlay.providerSource)
            overlay.providerSource.sendCommand("clipboard", "clear");
    }

    // A click anywhere outside the card closes this surface.
    //
    // The surface is the whole output, not the card: that is what makes an
    // outside click land here at all, and it is also what makes the panel
    // button that opened this close it in one click rather than two. While the
    // overlay is up the button is behind it, so the click never reaches the
    // panel, never re-enters `toggle()`, and focus returns exactly once.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        onPressed: overlay.dismissed()
    }

    Item {
        id: scene

        width: overlay.cardWidth
        height: overlay.cardHeight
        x: overlay.cardX
        y: overlay.cardY

        SoftOverlayCard {
            id: card

            ink: backdropInk
            // Escape closes the overlay whether or not the list is there to
            // hear it.
            focus: overlay.entries.length === 0
            Keys.onEscapePressed: overlay.dismissed()
            anchors.fill: parent
            reducedMotion: overlay.reducedMotion
            accessibleName: qsTr("Historial del portapapeles")
            attachedToTop: overlay.anchoredFromPanel
            openerRect: overlay.openerRect
            attachmentAnchorRect: overlay.attachmentAnchorRect
            attachmentStartY: overlay.attachmentStartY
            surfacePosition: Qt.point(overlay.cardX, overlay.cardY)

            Column {
                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceMd
                spacing: CelestinaTheme.spaceSm

                MenuHeader {
                    width: parent.width
                    ink: backdropInk
                    title: qsTr("Portapapeles")
                    subtitle: {
                        if (!overlay.offered)
                            return qsTr("No disponible");
                        if (overlay.truncated) {
                            return qsTr("%n entrada(s), lista parcial", "",
                                        overlay.entries.length);
                        }
                        return qsTr("%n entrada(s)", "", overlay.entries.length);
                    }
                    iconName: "clipboard-paste"

                    BackdropButton {
                        id: clearButton
                        ink: backdropInk
                        text: qsTr("Vaciar")
                        role: CelestinaButton.Destructive
                        enabled: overlay.entries.length > 0
                        onClicked: overlay.clear()
                    }
                }

                Item {
                    width: parent.width
                    height: parent.height - y

                    MenuSection { ink: backdropInk }

                    Text {
                        anchors.fill: parent
                        anchors.margins: CelestinaTheme.spaceLg
                        visible: !overlay.offered || overlay.entries.length === 0
                        text: !overlay.offered
                              ? qsTr("El historial del portapapeles no está disponible")
                              : qsTr("El portapapeles está vacío")
                        color: backdropInk.muted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontBody
                        wrapMode: Text.WordWrap
                    }

                    ListView {
                        id: entryList

                        anchors.fill: parent
                        anchors.margins: CelestinaTheme.spaceSm
                        clip: true
                        spacing: CelestinaTheme.spaceXs
                        visible: overlay.entries.length > 0
                        model: overlay.entries
                        currentIndex: overlay.currentIndex
                        onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
                        Accessible.role: Accessible.List
                        Accessible.name: qsTr("Entradas del historial")

                        // One cursor owns the list: arrows move, Enter selects,
                        // Delete removes and Escape dismisses.
                        Keys.onPressed: function(event) {
                            if (event.key === Qt.Key_Escape) {
                                overlay.dismissed();
                            } else if (event.key === Qt.Key_Down) {
                                if (overlay.entries.length > 0)
                                    overlay.currentIndex = Math.min(
                                        overlay.entries.length - 1,
                                        overlay.currentIndex + 1);
                            } else if (event.key === Qt.Key_Up) {
                                if (overlay.entries.length > 0)
                                    overlay.currentIndex = Math.max(
                                        0, overlay.currentIndex - 1);
                            } else if (event.key === Qt.Key_Return
                                       || event.key === Qt.Key_Enter) {
                                overlay.select(overlay.currentIndex);
                            } else if (event.key === Qt.Key_Delete
                                       || event.key === Qt.Key_Backspace) {
                                overlay.remove(overlay.currentIndex);
                            } else {
                                return;
                            }
                            event.accepted = true;
                        }

                        delegate: ClipboardEntryRow {
                            required property int index
                            required property var modelData

                            width: ListView.view.width
                            entry: modelData
                            current: overlay.currentIndex === index
                            ink: backdropInk
                            onSelected: {
                                overlay.currentIndex = index;
                                overlay.select(index);
                            }
                            onRemoved: {
                                overlay.currentIndex = index;
                                overlay.remove(index);
                            }
                        }
                    }
                }
            }
        }
    }
}
