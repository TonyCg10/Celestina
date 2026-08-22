// Every monitor that speaks DDC, each with its own brightness.
//
// The panel's own control can only reach the monitor its output is mapped to,
// which is the one thing it must not be able to do alone: a session with three
// screens had three separate places to change three brightnesses, and no place
// to see them together. This card is that place, and it is opened from any
// panel — the list is the session's, not this output's.
//
// A card rather than a menu because brightness is a level: it is moved, not
// chosen, and a slider inside a real `Menu` row fights that row's own
// click-to-activate.
//
// Three states are kept distinct, exactly as the provider publishes them. A
// monitor absent from the payload does not speak DDC and offers no brightness
// at all. A monitor present with a null level speaks it and has not answered:
// unknown, which is not darkness. A number was read back from the monitor.
//
// Rows are keyed by connector — the name the provider detected the bus as, and
// the same name the request is addressed with. Two monitors of the same model
// share a product name and never a connector.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import "ProviderReading.js" as ProviderReading

SoftCard {
    id: root

    required property var providerSource

    readonly property var brightness: ProviderReading.read(
                                              root.providerSource, "brightness")
    // Sorted so the list has a stable order across reads. A map's key order is
    // not a promise, and rows that reshuffle under the pointer are rows that
    // get dragged by accident.
    readonly property var connectors: {
        if (root.brightness === undefined)
            return [];
        return Object.keys(root.brightness).sort();
    }

    function levelOf(connector) {
        if (root.brightness === undefined
                || root.brightness[connector] === undefined
                || root.brightness[connector] === null)
            return -1;
        return root.brightness[connector];
    }

    function answered(connector) {
        return root.levelOf(connector) >= 0;
    }

    // Absolute, not a step: the slider knows where it was put, and a delta
    // would race the reading it was drawn from. The connector is always named
    // because one helper serves every panel, and `level` is the key the
    // session vocabulary already uses for an absolute one.
    function setLevel(connector, percent) {
        if (root.providerSource)
            root.providerSource.sendCommand(
                "brightness", "brightness-set",
                {"output": connector, "level": percent});
    }

    title: qsTr("Brillo")
    subtitle: root.connectors.length > 0
              ? qsTr("Monitores que responden a DDC")
              : qsTr("Ningún monitor responde a DDC")
    iconName: "sun"

    // Counted rather than listed: a `Repeater` handed a JavaScript array
    // rebuilds every delegate when that array is replaced, and this provider
    // publishes a new one on every reading. A monitor takes about a second to
    // answer over DDC, so a row rebuilt mid-drag is not a rare race — it is
    // most of them. The count changes only when a monitor comes or goes.
    Repeater {
        model: root.connectors.length

        delegate: LevelRow {
            id: monitorRow

            required property int index

            readonly property string connector:
                monitorRow.index < root.connectors.length
                ? root.connectors[monitorRow.index] : ""

            objectName: monitorRow.connector.length > 0
                        ? "celestina-level-row-" + monitorRow.connector : ""
            width: parent.width
            visible: monitorRow.connector.length > 0
            ink: root.ink
            label: monitorRow.connector
            iconName: "monitor"
            secondaryText: monitorRow.connector === root.outputName
                           ? qsTr("Esta pantalla") : ""
            level: Math.max(0, root.levelOf(monitorRow.connector))
            known: monitorRow.connector.length > 0
                   && root.answered(monitorRow.connector)
            onMoved: (target) => root.setLevel(monitorRow.connector, target)
        }
    }

    Text {
        width: parent.width
        visible: root.connectors.length === 0
        text: qsTr("Ningún monitor ofrece brillo por DDC")
        color: root.ink.faint
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontMini
        wrapMode: Text.WordWrap
    }
}
