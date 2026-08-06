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

    signal dismissed()

    readonly property int cardWidth: 460
    readonly property int cardHeight: 420

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

    Item {
        id: scene
        anchors.fill: parent

        GlassCard {
            id: card

            // Escape closes the overlay whether or not the list is there to
            // hear it.
            focus: overlay.entries.length === 0
            Keys.onEscapePressed: overlay.dismissed()
            anchors.fill: parent
            backdropSource: scene
            Accessible.role: Accessible.Dialog
            Accessible.name: qsTr("Historial del portapapeles")

            Column {
                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceLg
                spacing: CelestinaTheme.spaceMd

                Row {
                    id: headerRow

                    width: parent.width
                    spacing: CelestinaTheme.spaceSm

                    Text {
                        width: parent.width - clearButton.width - parent.spacing
                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("Historial del portapapeles")
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowTitle
                        font.weight: CelestinaTheme.weightDemiBold
                        elide: Text.ElideRight
                    }

                    CelestinaButton {
                        id: clearButton
                        text: qsTr("Vaciar")
                        role: CelestinaButton.Destructive
                        enabled: overlay.entries.length > 0
                        onClicked: overlay.clear()
                    }
                }

                Text {
                    width: parent.width
                    visible: !overlay.offered
                    text: qsTr("El historial del portapapeles no está disponible")
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                }

                Text {
                    width: parent.width
                    visible: overlay.offered && overlay.entries.length === 0
                    text: qsTr("El portapapeles está vacío")
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                }

                ListView {
                    id: entryList

                    width: parent.width
                    height: parent.height - headerRow.height - parent.spacing
                    clip: true
                    spacing: 2
                    visible: overlay.entries.length > 0
                    model: overlay.entries
                    currentIndex: overlay.currentIndex
                    onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
                    Accessible.role: Accessible.List
                    Accessible.name: qsTr("Entradas del historial")

                    // Only one focusable widget besides the clear button, the
                    // same single-cursor keyboard model `OpenWithDialog` uses:
                    // arrows move the highlight, Enter re-selects it, Delete
                    // removes it, Escape closes the overlay.
                    Keys.onPressed: function(event) {
                        if (event.key === Qt.Key_Escape) {
                            overlay.dismissed();
                        } else if (event.key === Qt.Key_Down) {
                            if (overlay.entries.length > 0)
                                overlay.currentIndex =
                                        Math.min(overlay.entries.length - 1,
                                                 overlay.currentIndex + 1);
                        } else if (event.key === Qt.Key_Up) {
                            if (overlay.entries.length > 0)
                                overlay.currentIndex = Math.max(0, overlay.currentIndex - 1);
                        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
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
                        // What a click means belongs to the overlay: the row
                        // reports, and the provider decides.
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
