pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.hematita 1.0

// Rendimiento: every resource on the left, the chosen one on the right. Rows
// are woven from the adapter's index-aligned lists on each revision, and the
// selection is a key, so a disk that comes and goes never moves it. Every
// word a person reads is composed here from tokens.
Item {
    id: page

    // `resources` is taken: QQuickItem already owns that name for its
    // non-visual children.
    required property HematitaResources metrics

    property string selectedKey: "cpu"
    // The woven rows: { key, kind, label, state, reasonKind, reasonPath,
    // load, numbers, history }.
    property var rows: []
    readonly property var selectedRow: page.rowFor(page.selectedKey)
    readonly property real listFraction: 0.32
    // What a delegate reads while its index is momentarily out of the woven
    // array: a row-shaped nothing, rather than an undefined every binding
    // would have to guard.
    readonly property var emptyRow: ({ key: "", kind: "", label: "", state: "waiting",
                                       reasonKind: "", reasonPath: "", load: "normal",
                                       numbers: [], history: [] })

    function rowAt(index) {
        return index >= 0 && index < page.rows.length ? page.rows[index] : page.emptyRow
    }

    function indexOf(key) {
        for (let index = 0; index < page.rows.length; ++index)
            if (page.rows[index].key === key)
                return index
        return -1
    }

    function rowFor(key) {
        for (let index = 0; index < page.rows.length; ++index)
            if (page.rows[index].key === key)
                return page.rows[index]
        return null
    }

    function weave() {
        const keys = page.metrics.resourceKeys
        const kinds = page.metrics.resourceKinds
        const labels = page.metrics.resourceLabels
        const states = page.metrics.resourceStates
        const reasonKinds = page.metrics.resourceReasonKinds
        const reasonPaths = page.metrics.resourceReasonPaths
        const loads = page.metrics.resourceLoads
        const numbers = page.metrics.resourceNumbers
        const histories = page.metrics.resourceHistories
        // Defensive: a short column would mean a publication error, and fewer
        // rows are better than rows with undefined fields.
        const count = Math.min(keys.length, kinds.length, labels.length, states.length,
                               reasonKinds.length, reasonPaths.length, loads.length,
                               numbers.length, histories.length)
        const woven = []
        for (let index = 0; index < count; ++index)
            woven.push({ key: keys[index], kind: kinds[index], label: labels[index],
                         state: states[index], reasonKind: reasonKinds[index],
                         reasonPath: reasonPaths[index], load: loads[index],
                         numbers: numbers[index], history: histories[index] })
        page.rows = woven
        if (page.rowFor(page.selectedKey) === null && woven.length > 0)
            page.selectedKey = woven[0].key
    }

    Connections {
        target: page.metrics
        function onRevisionChanged() { page.weave() }
    }

    Component.onCompleted: page.weave()

    // ── Words ──────────────────────────────────────────────────────────
    function nameFor(row) {
        switch (row.kind) {
        case "cpu": return qsTr("Procesador")
        case "memory": return qsTr("Memoria")
        case "gpu": return qsTr("Gráfica")
        case "disk": return row.label
        case "network": return row.label.length > 0 ? row.label : qsTr("Red")
        }
        return row.key
    }

    function subtitleFor(row) {
        switch (row.kind) {
        case "cpu": return row.label
        // The card's PCI identifier is a key, not something to read; the
        // name already says which resource this is.
        case "gpu": return ""
        case "disk": return row.key.substring(5)
        case "network": return row.state === "ready"
                               ? (row.numbers[3] === 1 ? qsTr("Inalámbrica") : qsTr("Cable"))
                               : ""
        }
        return ""
    }

    function reasonFor(row) {
        switch (row.reasonKind) {
        case "unreadable": return qsTr("No se pudo leer %1").arg(row.reasonPath)
        case "malformed": return qsTr("Contenido inesperado en %1").arg(row.reasonPath)
        case "no-rate": return qsTr("Sin variación entre dos lecturas de %1").arg(row.reasonPath)
        }
        return ""
    }

    function percentText(value) {
        return Math.round(value) + " %"
    }

    function gib(kib) {
        return (kib / 1048576).toLocaleString(Qt.locale(), "f", 1) + " GiB"
    }

    function bytesText(bytes) {
        if (bytes >= 1073741824) return (bytes / 1073741824).toLocaleString(Qt.locale(), "f", 1) + " GiB"
        if (bytes >= 1048576) return (bytes / 1048576).toLocaleString(Qt.locale(), "f", 1) + " MiB"
        if (bytes >= 1024) return (bytes / 1024).toLocaleString(Qt.locale(), "f", 0) + " KiB"
        return Math.round(bytes) + " B"
    }

    function rateText(bytesPerSecond) {
        return page.bytesText(bytesPerSecond) + "/s"
    }

    // The value beside the name in the side list.
    function valueFor(row) {
        if (row.state === "unavailable") return qsTr("No disponible")
        if (row.state === "waiting") return "—"
        const n = row.numbers
        switch (row.kind) {
        case "cpu": return page.percentText(n[0])
        case "memory": return page.gib(n[0]) + " / " + page.gib(n[1])
        case "gpu": return page.percentText(n[0])
        case "disk": return "↓ " + page.rateText(n[0]) + "  ↑ " + page.rateText(n[1])
        case "network": return "↓ " + page.rateText(n[0]) + "  ↑ " + page.rateText(n[1])
        }
        return ""
    }

    // Flat list of alternating label, value strings for the detail.
    function factsFor(row) {
        if (row === null || row.state !== "ready") return []
        const n = row.numbers
        switch (row.kind) {
        case "cpu":
            return [qsTr("Uso"), page.percentText(n[0]),
                    qsTr("Frecuencia"), n[1] > 0 ? (n[1] / 1000).toLocaleString(Qt.locale(), "f", 2) + " GHz" : "—",
                    qsTr("Núcleos"), String(n[2])]
        case "memory":
            return [qsTr("En uso"), page.gib(n[0]),
                    qsTr("Total"), page.gib(n[1]),
                    qsTr("Intercambio"), page.gib(n[2]) + " / " + page.gib(n[3])]
        case "gpu":
            return [qsTr("Uso"), page.percentText(n[0]),
                    qsTr("Memoria ocupada"), page.percentText(n[1]),
                    qsTr("VRAM"), page.bytesText(n[2]) + " / " + page.bytesText(n[3]),
                    qsTr("GTT"), page.bytesText(n[4]) + " / " + page.bytesText(n[5]),
                    qsTr("Reloj"), n[6] > 0 ? Math.round(n[6]) + " MHz" : "—",
                    qsTr("Reloj de memoria"), n[7] > 0 ? Math.round(n[7]) + " MHz" : "—"]
        case "disk":
            return [qsTr("Lectura"), page.rateText(n[0]),
                    qsTr("Escritura"), page.rateText(n[1]),
                    qsTr("Capacidad"), page.bytesText(n[2]),
                    qsTr("Tipo"), n[3] === 1 ? qsTr("Disco mecánico") : qsTr("Estado sólido")]
        case "network":
            return [qsTr("Recibido"), page.rateText(n[0]),
                    qsTr("Enviado"), page.rateText(n[1]),
                    qsTr("Velocidad"), n[2] > 0 ? Math.round(n[2]) + " Mbit/s" : "—",
                    qsTr("Estado"), n[4] === 1 ? qsTr("Activa") : qsTr("Inactiva")]
        }
        return []
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: CelestinaTheme.spaceSm

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: CelestinaTheme.spaceLg

            CelestinaSurface {
                Layout.preferredWidth: page.width * page.listFraction
                Layout.fillHeight: true
                role: CelestinaSurface.Panel
                padding: CelestinaTheme.spaceXs

                // The model is the row COUNT, not the woven array: a new
                // array every second would reset the view, its delegates and
                // its scroll once a second. The bindings below re-evaluate
                // when `page.rows` is reassigned, so only a machine that
                // gained or lost a resource rebuilds anything.
                contentItem: ListView {
                    id: list
                    clip: true
                    spacing: CelestinaTheme.spaceXs
                    model: page.rows.length
                    // The view is the keyboard's way in: Tab lands here, arrows
                    // move the selection, and the selection is the current
                    // item, so the screen reader follows it. Off-screen rows
                    // are reached by moving the current index, which scrolls.
                    activeFocusOnTab: true
                    keyNavigationEnabled: true
                    currentIndex: page.indexOf(page.selectedKey)
                    onCurrentIndexChanged: {
                        if (currentIndex >= 0 && currentIndex < page.rows.length)
                            page.selectedKey = page.rows[currentIndex].key
                    }
                    highlightFollowsCurrentItem: true
                    Accessible.role: Accessible.List
                    Accessible.name: qsTr("Recursos")
                    delegate: ResourceRow {
                        id: resourceRow
                        required property int index
                        readonly property var row: page.rowAt(resourceRow.index)
                        width: list.width
                        name: page.nameFor(resourceRow.row)
                        value: page.valueFor(resourceRow.row)
                        series: resourceRow.row.history
                        load: resourceRow.row.load
                        selected: resourceRow.row.key === page.selectedKey
                        onClicked: page.selectedKey = resourceRow.row.key
                    }
                }
            }

            ResourceDetail {
                Layout.fillWidth: true
                Layout.fillHeight: true
                title: page.selectedRow ? page.nameFor(page.selectedRow) : ""
                subtitle: page.selectedRow ? page.subtitleFor(page.selectedRow) : ""
                series: page.selectedRow ? page.selectedRow.history : []
                load: page.selectedRow ? page.selectedRow.load : "normal"
                facts: page.factsFor(page.selectedRow)
                coreHistories: page.selectedRow && page.selectedRow.kind === "cpu"
                               ? page.metrics.cpuCoreHistories : []
                notice: page.selectedRow && page.selectedRow.state === "unavailable"
                        ? page.reasonFor(page.selectedRow) : ""
            }
        }

        // The failure line lives in the column, under the two panels, never
        // over them. The sampling thread failing to start is the one failure
        // no row can carry.
        Text {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            visible: page.metrics.startFailed
            text: qsTr("No se pudo iniciar la lectura del sistema")
            color: CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }
    }
}
