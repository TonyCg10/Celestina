// Live CPU and memory readings, each of which opens the system monitor the
// session already uses.
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

    // The row structure depends only on whether a reading exists at all. The
    // live values deliberately do not appear here: a fresh list on every
    // provider tick made the Instantiator tear down and recreate every row
    // once per second, and on the live compositor the card was re-measured
    // against a menu that was mid-rebuild, leaving it permanently short — the
    // clipped `Rendimiento` the author recorded twice. Rows read their moving
    // values by binding instead, so a tick changes two labels and nothing
    // else.
    readonly property var entries: {
        const built = [
            {"kind": "header", "text": qsTr("Rendimiento")},
            {"kind": "section", "text": qsTr("Uso actual")}
        ];

        if (root.hasReading) {
            built.push({"kind": "metric", "text": qsTr("Procesador"),
                        "icon": "cpu", "metric": "cpu"});
            built.push({"kind": "metric", "text": qsTr("Memoria"),
                        "icon": "memory-stick", "metric": "ram"});
        } else {
            built.push({"kind": "unavailable",
                        "text": qsTr("Sin lectura de rendimiento")});
        }

        return built;
    }

    // The moving part, read live by each metric row. A vanished reading also
    // flips `hasReading`, which is a real structure change and the one thing
    // that may rebuild the rows.
    function metricValue(metric) {
        if (!root.hasReading)
            return 0;
        return metric === "cpu" ? root.performance.cpu
                                : root.performance.ram;
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

            ink: root.ink
            headerTrailingGap: entry.isHeader ? root.headerBodyGap : 0
            verticalInset: root.rowVerticalInset
            trailingGap: entry.isHeader ? 0 : root.itemSpacing
            text: entry.modelData.text
            header: entry.isHeader
            sectionLabel: entry.isSection
            subtitle: entry.isHeader ? root.readingLine : ""
            iconName: entry.isHeader || entry.isMetric ? entry.modelData.icon || "cpu"
                      : entry.isUnavailable ? "circle-alert" : ""
            // A reading is its own way in: the thing being measured opens the
            // monitor that measures it. That replaces the separate tools
            // section, whose single row was the only reason this menu had a
            // trailing section at all.
            actionable: entry.isMetric
            note: entry.isMetric
                  ? qsTr("%1 %").arg(root.metricValue(entry.modelData.metric))
                  : ""
            noteColor: root.ink.primary
            Accessible.description: entry.isMetric
                                    ? qsTr("Abre el monitor del sistema") : ""
            onTriggered: {
                if (entry.isMetric)
                    root.openMonitor();
            }
        }
    }
}
