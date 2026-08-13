// Every device Magnetita reports, and the three things the daemon can be asked
// to do to one: ring it, pair with it, drop the pairing.
//
// The provider here is the `DevicesClient`, not the aggregate helper bridge:
// the phone speaks D-Bus through Magnetita and nothing else. Everything shown
// is the daemon's own snapshot and nothing is painted optimistically — an
// action's outcome arrives as the next `Changed` snapshot or not at all, which
// is the same rule the panel reading follows.
//
// Rows are identified by `id` — the daemon's device identity — and never by
// name: two phones may share a product name, and a pairing dropped by name
// would be a coin toss between them.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

SoftMenu {
    id: root

    // The DevicesClient, or null in a harness. `var` rather than the concrete
    // type so a test can pass nothing and the menu still names its empty state.
    required property var providerSource

    title: qsTr("Móvil")
    itemSpacing: CelestinaTheme.spaceSm
    headerBodyGap: CelestinaTheme.spaceMd
    rowVerticalInset: CelestinaTheme.spaceXs

    readonly property var devices: root.providerSource
                                   && root.providerSource.devices !== undefined
                                   ? root.providerSource.devices : []
    readonly property string readingLine: root.devices.length > 0
                                          ? qsTr("%n dispositivo(s)", "",
                                                 root.devices.length)
                                          : qsTr("Ningún dispositivo a la vista")

    // Structure only: identity, name and the states that decide which actions
    // a row offers. The battery deliberately stays out — it is read live by
    // each row, so a battery tick changes one label instead of rebuilding the
    // list under the pointer (the clipped-menu defect PerformanceMenu records).
    readonly property var entries: {
        const built = [
            {"kind": "header", "text": qsTr("Móvil")},
            {"kind": "section", "text": qsTr("Dispositivos")}
        ];

        if (root.devices.length === 0) {
            built.push({"kind": "unavailable",
                        "text": qsTr("Nada conectado ni visible ahora")});
            return built;
        }

        for (const device of root.devices) {
            built.push({
                "kind": "device",
                "id": device.id !== undefined ? device.id : "",
                "name": device.name !== undefined && device.name.length > 0
                        ? device.name : qsTr("Dispositivo sin nombre"),
                "connected": device.connected === true,
                "paired": device.paired === true,
                "mounted": device.mounted === true
            });
        }

        return built;
    }

    // The moving part, read live per row.
    function batteryOf(deviceId) {
        for (const device of root.devices) {
            if (device.id === deviceId) {
                const battery = device.battery !== undefined ? device.battery : -1;
                return battery >= 0 ? battery : -1;
            }
        }
        return -1;
    }

    function stateLine(entry) {
        if (!entry.connected)
            return qsTr("Recordado, ahora fuera de alcance");
        if (!entry.paired)
            return qsTr("A la vista, sin emparejar");
        return entry.mounted ? qsTr("Conectado y montado")
                             : qsTr("Conectado");
    }

    Instantiator {
        model: root.entries
        onObjectAdded: (index, object) => root.menu.insertItem(index, object)
        onObjectRemoved: (index, object) => root.menu.removeItem(object)

        delegate: SoftMenuRow {
            id: entry

            required property var modelData

            readonly property bool isHeader: entry.modelData.kind === "header"
            readonly property bool isSection: entry.modelData.kind === "section"
            readonly property bool isDevice: entry.modelData.kind === "device"
            readonly property bool isUnavailable:
                entry.modelData.kind === "unavailable"
            readonly property int battery: entry.isDevice
                                           ? root.batteryOf(entry.modelData.id)
                                           : -1

            ink: root.ink
            headerTrailingGap: entry.isHeader ? root.headerBodyGap : 0
            verticalInset: root.rowVerticalInset
            trailingGap: entry.isHeader ? 0 : root.itemSpacing
            text: entry.isDevice ? entry.modelData.name : entry.modelData.text
            header: entry.isHeader
            sectionLabel: entry.isSection
            subtitle: entry.isHeader ? root.readingLine
                      : entry.isDevice ? root.stateLine(entry.modelData) : ""
            iconName: entry.isHeader || entry.isDevice ? "phone"
                      : entry.isUnavailable ? "circle-alert" : ""
            // A device row is not a choice — there is nothing to activate by
            // clicking a phone's name. The actions live on the trailing edge.
            actionable: false
            note: entry.isDevice && entry.battery >= 0
                  ? qsTr("%1 %").arg(entry.battery) : ""
            noteColor: root.ink.primary

            // Ring: only a device that can hear it offers it.
            trailingPrimaryIcon: entry.isDevice ? "bell" : ""
            trailingPrimaryHelpText: qsTr("Hacer sonar %1").arg(entry.text)
            trailingPrimaryEnabled: entry.isDevice && entry.modelData.connected
            onTrailingPrimaryTriggered: {
                if (root.providerSource)
                    root.providerSource.ring(entry.modelData.id);
            }

            // Pair or unpair, whichever applies to this row's own state.
            trailingSecondaryIcon: entry.isDevice
                                   ? (entry.modelData.paired ? "unplug" : "key")
                                   : ""
            trailingSecondaryHelpText: entry.isDevice && entry.modelData.paired
                                       ? qsTr("Desvincular %1").arg(entry.text)
                                       : qsTr("Emparejar %1").arg(entry.text)
            trailingSecondaryEnabled: entry.isDevice
                                      && (entry.modelData.paired
                                          || entry.modelData.connected)
            onTrailingSecondaryTriggered: {
                if (!root.providerSource)
                    return;
                if (entry.modelData.paired)
                    root.providerSource.unpair(entry.modelData.id);
                else
                    root.providerSource.requestPair(entry.modelData.id);
            }

            Accessible.description: entry.isDevice
                                    ? root.stateLine(entry.modelData) : ""
        }
    }
}
