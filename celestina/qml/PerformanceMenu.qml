// Live CPU and memory readings, plus the system monitor the session already
// uses.
//
// This menu starts no process itself. ProviderReading keeps its values tied to
// the aggregate helper's revision, and the existing `sysmon/open-monitor`
// request remains the only route that launches Mission Center.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import "ProviderReading.js" as ProviderReading

SoftMenu {
    id: root

    required property var providerSource

    readonly property var performance: ProviderReading.read(
                                               root.providerSource, "sysmon")
    readonly property bool hasReading: root.performance !== undefined
                                       && root.performance.cpu !== undefined
                                       && root.performance.ram !== undefined
    readonly property string readingLine: root.hasReading
                                             ? qsTr("Uso actual del sistema")
                                             : qsTr("Sin lectura actual")

    title: qsTr("Rendimiento")
    itemSpacing: CelestinaTheme.spaceSm
    headerBodyGap: CelestinaTheme.spaceMd
    rowVerticalInset: CelestinaTheme.spaceXs

    readonly property var entries: {
        const built = [
            {"kind": "header", "text": qsTr("Rendimiento"),
             "subtitle": root.readingLine},
            {"kind": "section", "text": qsTr("Uso actual")}
        ];

        if (root.hasReading) {
            built.push({"kind": "metric", "text": qsTr("Procesador"),
                        "icon": "cpu", "value": root.performance.cpu});
            built.push({"kind": "metric", "text": qsTr("Memoria"),
                        "icon": "memory-stick", "value": root.performance.ram});
        } else {
            built.push({"kind": "unavailable",
                        "text": qsTr("Sin lectura de rendimiento")});
        }

        built.push({"kind": "section", "text": qsTr("Herramientas")});
        built.push({"kind": "monitor",
                    "text": qsTr("Abrir el monitor del sistema")});
        return built;
    }

    function openMonitor() {
        if (root.providerSource)
            root.providerSource.sendCommand("sysmon", "open-monitor");
        root.menu.close();
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
            readonly property bool isMetric: entry.modelData.kind === "metric"
            readonly property bool isUnavailable:
                entry.modelData.kind === "unavailable"
            readonly property bool isMonitor: entry.modelData.kind === "monitor"

            ink: root.ink
            headerTrailingGap: entry.isHeader ? root.headerBodyGap : 0
            verticalInset: root.rowVerticalInset
            trailingGap: entry.isHeader ? 0 : root.itemSpacing
            text: entry.modelData.text
            header: entry.isHeader
            sectionLabel: entry.isSection
            subtitle: entry.isHeader ? entry.modelData.subtitle : ""
            iconName: entry.isHeader || entry.isMetric ? entry.modelData.icon || "cpu"
                      : entry.isUnavailable ? "circle-alert"
                      : entry.isMonitor ? "app-window" : ""
            actionable: entry.isMonitor
            note: entry.isMetric ? qsTr("%1 %").arg(entry.modelData.value) : ""
            noteColor: root.ink.primary
            Accessible.description: entry.isMonitor
                                    ? qsTr("Abre Mission Center") : ""
            onTriggered: {
                if (entry.isMonitor)
                    root.openMonitor();
            }
        }
    }
}
