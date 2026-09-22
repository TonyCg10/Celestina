import QtQuick
import QtQuick.Layouts
import org.celestina.hematita 1.0

// Rendimiento: the resources on the left, the chosen one on the right. In H1
// the list is CPU and memory; H2 makes it a model when disks and interfaces
// bring a count nobody can write down in advance.
Item {
    id: page

    // `resources` is taken: QQuickItem already owns that name for its
    // non-visual children.
    required property HematitaResources metrics

    // 0 = CPU, 1 = memory.
    property int selected: 0

    function gib(kib) {
        return (kib / 1048576).toLocaleString(Qt.locale(), "f", 1) + " GiB"
    }

    function percent(value) {
        return value < 0 ? "—" : value + " %"
    }

    readonly property string cpuValue: page.percent(page.metrics.cpuPercent)
    // Before the first snapshot the totals are zero, and "0.0 GiB / 0.0 GiB"
    // is a reading nobody took. Say nothing instead, as `percent` does.
    readonly property string memoryValue: page.metrics.memoryTotalKib <= 0
                                          ? "—"
                                          : page.gib(page.metrics.memoryUsedKib)
                                            + " / " + page.gib(page.metrics.memoryTotalKib)

    RowLayout {
        anchors.fill: parent
        spacing: CelestinaTheme.spaceLg

        CelestinaSurface {
            Layout.preferredWidth: page.width * 0.32
            Layout.fillHeight: true
            role: CelestinaSurface.Panel
            padding: CelestinaTheme.spaceXs

            contentItem: Column {
                id: list
                spacing: CelestinaTheme.spaceXs

                ResourceRow {
                    width: list.width
                    name: qsTr("Procesador")
                    value: page.cpuValue
                    series: page.metrics.cpuHistory
                    load: page.metrics.cpuLoad
                    selected: page.selected === 0
                    onClicked: page.selected = 0
                }
                ResourceRow {
                    width: list.width
                    name: qsTr("Memoria")
                    value: page.memoryValue
                    series: page.metrics.memoryHistory
                    load: page.metrics.memoryLoad
                    selected: page.selected === 1
                    onClicked: page.selected = 1
                }
            }
        }

        ResourceDetail {
            Layout.fillWidth: true
            Layout.fillHeight: true
            title: page.selected === 0 ? qsTr("Procesador") : qsTr("Memoria")
            subtitle: page.selected === 0 ? page.metrics.cpuModel : ""
            series: page.selected === 0 ? page.metrics.cpuHistory : page.metrics.memoryHistory
            load: page.selected === 0 ? page.metrics.cpuLoad : page.metrics.memoryLoad
            facts: page.selected === 0
                   ? [qsTr("Uso"), page.cpuValue,
                      qsTr("Frecuencia"), page.metrics.cpuFrequencyMhz > 0
                          ? (page.metrics.cpuFrequencyMhz / 1000).toLocaleString(Qt.locale(), "f", 2) + " GHz"
                          : "—",
                      qsTr("Núcleos"), String(page.metrics.cpuCores)]
                   : [qsTr("En uso"), page.gib(page.metrics.memoryUsedKib),
                      qsTr("Total"), page.gib(page.metrics.memoryTotalKib),
                      qsTr("Intercambio"), page.gib(page.metrics.swapUsedKib)
                          + " / " + page.gib(page.metrics.swapTotalKib)]
        }
    }

    // The truthful empty state: a source that could not be read says so in
    // the page rather than freezing the last number.
    Text {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceSm
        visible: !page.metrics.available && page.metrics.unavailableReason.length > 0
        text: qsTr("No se pudo leer: ") + page.metrics.unavailableReason
        color: CelestinaTheme.danger
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
    }
}
