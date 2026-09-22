pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.hematita 1.0

// The table both pages share: a bar with the search and the actions, the
// column titles, and the rows. `grouped` puts each application's row above
// its processes and lets it fold them.
Item {
    id: table

    required property HematitaProcesses processes
    required property bool grouped
    // What the kill dialog blurs beneath itself.
    required property Item backdrop

    property int selectedPid: -1
    property var rows: []
    property var groups: []
    // Application ids folded shut.
    property var collapsed: ({})
    // The visible sequence: in the flat layout every row; grouped, each
    // application followed by its rows unless folded. Each entry is
    // { kind: "group"|"process", index }.
    property var entries: []

    // Row-shaped and group-shaped nothings, so a delegate whose index is
    // momentarily out of the woven arrays reads fields rather than undefined.
    readonly property var emptyEntry: ({ kind: "process", index: -1 })
    readonly property var emptyRow: ({ pid: -1, name: "", user: "", cpu: 0, memory: 0,
                                       read: 0, write: 0, application: "", group: -1,
                                       actionable: false })
    readonly property var emptyGroup: ({ id: "", name: "", icon: "", cpu: 0, memory: 0, count: 0 })

    readonly property var columns: [
        { field: "name", title: qsTr("Nombre"), width: Math.max(160, table.width * 0.34), numeric: false },
        { field: "user", title: qsTr("Usuario"), width: 90, numeric: false },
        { field: "pid", title: qsTr("PID"), width: 80, numeric: true },
        { field: "cpu", title: qsTr("CPU"), width: 80, numeric: true },
        { field: "memory", title: qsTr("Memoria"), width: 110, numeric: true },
        { field: "read", title: qsTr("Lectura"), width: 110, numeric: true },
        { field: "write", title: qsTr("Escritura"), width: 110, numeric: true }
    ]

    function percentText(value) {
        return value.toLocaleString(Qt.locale(), "f", 1) + " %"
    }

    function bytesText(bytes) {
        if (bytes >= 1073741824) return (bytes / 1073741824).toLocaleString(Qt.locale(), "f", 1) + " GiB"
        if (bytes >= 1048576) return (bytes / 1048576).toLocaleString(Qt.locale(), "f", 1) + " MiB"
        if (bytes >= 1024) return (bytes / 1024).toLocaleString(Qt.locale(), "f", 0) + " KiB"
        return Math.round(bytes) + " B"
    }

    function rateText(value) {
        return value > 0 ? table.bytesText(value) + "/s" : "—"
    }

    function weave() {
        const published = table.processes
        // Defensive: a short column would mean a publication error, and fewer
        // rows are better than rows with undefined fields.
        const count = Math.min(published.processPids.length, published.processNames.length,
                               published.processUsers.length, published.processCpuPercents.length,
                               published.processMemoryKib.length, published.processReadRates.length,
                               published.processWriteRates.length, published.processApplications.length,
                               published.processGroupIndices.length, published.processActionable.length)
        const woven = []
        for (let index = 0; index < count; ++index)
            woven.push({ pid: published.processPids[index], name: published.processNames[index],
                         user: published.processUsers[index], cpu: published.processCpuPercents[index],
                         memory: published.processMemoryKib[index], read: published.processReadRates[index],
                         write: published.processWriteRates[index],
                         application: published.processApplications[index],
                         group: published.processGroupIndices[index],
                         actionable: published.processActionable[index] === 1 })
        const groupCount = Math.min(published.groupIds.length, published.groupNames.length,
                                    published.groupIcons.length, published.groupCpuPercents.length,
                                    published.groupMemoryKib.length, published.groupCounts.length)
        const wovenGroups = []
        for (let index = 0; index < groupCount; ++index)
            wovenGroups.push({ id: published.groupIds[index], name: published.groupNames[index],
                               icon: published.groupIcons[index], cpu: published.groupCpuPercents[index],
                               memory: published.groupMemoryKib[index], count: published.groupCounts[index] })
        table.rows = woven
        table.groups = wovenGroups
        table.entries = table.layout()
    }

    function layout() {
        const out = []
        if (!table.grouped) {
            for (let index = 0; index < table.rows.length; ++index)
                out.push({ kind: "process", index: index })
            return out
        }
        for (let group = 0; group < table.groups.length; ++group) {
            out.push({ kind: "group", index: group })
            if (table.collapsed[table.groups[group].id])
                continue
            for (let index = 0; index < table.rows.length; ++index)
                if (table.rows[index].group === group)
                    out.push({ kind: "process", index: index })
        }
        return out
    }

    function toggleGroup(id) {
        const next = Object.assign({}, table.collapsed)
        if (next[id])
            delete next[id]
        else
            next[id] = true
        table.collapsed = next
        table.entries = table.layout()
    }

    // The entry index showing `pid`, or -1. The selection is a pid and the
    // list's cursor is an index into `entries`, so each has to be able to
    // name the other.
    function entryOfPid(pid) {
        for (let index = 0; index < table.entries.length; ++index) {
            const entry = table.entries[index]
            if (entry.kind === "process" && entry.index < table.rows.length
                && table.rows[entry.index].pid === pid)
                return index
        }
        return -1
    }

    // The pid an entry shows, or -1 for an application row.
    function pidOfEntry(index) {
        if (index < 0 || index >= table.entries.length)
            return -1
        const entry = table.entries[index]
        if (entry.kind !== "process" || entry.index < 0 || entry.index >= table.rows.length)
            return -1
        return table.rows[entry.index].pid
    }

    function selectedRow() {
        for (let index = 0; index < table.rows.length; ++index)
            if (table.rows[index].pid === table.selectedPid)
                return table.rows[index]
        return null
    }

    function outcomeText() {
        const published = table.processes
        if (published.actionOutcome === "")
            return ""
        const verb = published.actionKind === "kill" ? qsTr("Matar") : qsTr("Terminar")
        switch (published.actionOutcome) {
        case "done":
            return qsTr("%1: señal enviada al proceso %2").arg(verb).arg(published.actionPid)
        case "refused":
            return qsTr("%1: el proceso %2 no es tuyo; actuar sobre él llega con la fase de servicios")
                     .arg(verb).arg(published.actionPid)
        case "failed":
            return qsTr("%1: el sistema rechazó la señal para el proceso %2")
                     .arg(verb).arg(published.actionPid)
        }
        return ""
    }

    // A click writes `selectedPid` and the arrow keys write `currentIndex`;
    // each writes the other back, so the cursor and the selection are one
    // thing however they were moved.
    onSelectedPidChanged: {
        const index = table.entryOfPid(table.selectedPid)
        if (index >= 0)
            list.currentIndex = index
        // A new selection is a new question; the last action's answer is not
        // about this row.
        table.processes.clearAction()
    }

    Connections {
        target: table.processes
        function onRevisionChanged() { table.weave() }
    }

    Component.onCompleted: table.weave()

    ColumnLayout {
        anchors.fill: parent
        spacing: CelestinaTheme.spaceSm

        // ── Bar ────────────────────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            spacing: CelestinaTheme.spaceSm

            CelestinaTextField {
                id: search

                Layout.preferredWidth: 260
                shape: CelestinaTextField.Search
                placeholderText: qsTr("Buscar por nombre, PID o aplicación")
                Accessible.name: search.placeholderText
                onTextChanged: {
                    table.processes.filterText = search.text
                    table.processes.refresh()
                }
            }

            Text {
                text: table.processes.shownCount === table.processes.totalCount
                      ? qsTr("%1 procesos").arg(table.processes.totalCount)
                      : qsTr("%1 de %2 procesos").arg(table.processes.shownCount)
                                                 .arg(table.processes.totalCount)
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                font.features: CelestinaTheme.fontFeaturesTabular
            }

            Item { Layout.fillWidth: true }

            CelestinaCapsule {
                CelestinaIconButton {
                    iconName: "circle-stop"
                    helpText: qsTr("Terminar el proceso seleccionado")
                    role: CelestinaButton.Ghost
                    enabled: table.selectedRow() !== null && table.selectedRow().actionable
                    onClicked: table.processes.terminate(table.selectedPid)
                }

                CelestinaIconButton {
                    iconName: "x"
                    helpText: qsTr("Matar el proceso seleccionado")
                    role: CelestinaButton.Ghost
                    enabled: table.selectedRow() !== null && table.selectedRow().actionable
                    onClicked: killDialog.ask(table.selectedPid, table.selectedRow().name)
                }
            }
        }

        // ── What the table has to say, if anything ─────────────────────
        Text {
            id: note

            Layout.fillWidth: true
            visible: note.text.length > 0
            text: {
                const row = table.selectedRow()
                if (row !== null && !row.actionable)
                    return qsTr("El proceso %1 pertenece a %2; terminarlo o matarlo llega con la fase de servicios")
                             .arg(row.pid).arg(row.user)
                if (!table.processes.available)
                    return qsTr("No se pudo leer %1").arg(table.processes.reasonPath)
                return table.outcomeText()
            }
            color: table.processes.available && table.processes.actionOutcome !== "failed"
                   ? CelestinaTheme.textMuted : CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideRight
        }

        // ── Header ─────────────────────────────────────────────────────
        ProcessHeader {
            Layout.fillWidth: true
            columns: table.columns
            sortField: table.processes.sortField
            sortAscending: table.processes.sortAscending
            onSortRequested: function(field) {
                // The user column names its rows; it does not order them.
                if (field === "user")
                    return
                if (table.processes.sortField === field) {
                    table.processes.sortAscending = !table.processes.sortAscending
                } else {
                    table.processes.sortField = field
                    table.processes.sortAscending = field === "name" || field === "pid"
                }
                table.processes.refresh()
            }
        }

        // ── Rows ───────────────────────────────────────────────────────
        CelestinaSurface {
            Layout.fillWidth: true
            Layout.fillHeight: true
            role: CelestinaSurface.Panel
            padding: CelestinaTheme.spaceXs

            contentItem: Item {
                ListView {
                    id: list

                    anchors.fill: parent
                    clip: true
                    model: table.entries.length
                    activeFocusOnTab: true
                    keyNavigationEnabled: true
                    Accessible.role: Accessible.List
                    Accessible.name: table.grouped ? qsTr("Aplicaciones") : qsTr("Procesos")

                    // Landing on an application row selects nothing and folds
                    // nothing: it is a place in the list, and Space on the row
                    // itself is what opens it.
                    onCurrentIndexChanged: {
                        const pid = table.pidOfEntry(list.currentIndex)
                        if (pid >= 0)
                            table.selectedPid = pid
                    }

                    // Both row shapes are built and one of them is shown. A
                    // `Loader` with its components declared beside it cannot see
                    // the delegate's own id under `ComponentBehavior: Bound`, and
                    // the answer is a static delegate rather than a suppression.
                    delegate: Item {
                        id: slot

                        required property int index

                        readonly property var entry: slot.index >= 0 && slot.index < table.entries.length
                                                     ? table.entries[slot.index] : table.emptyEntry
                        readonly property bool isGroup: slot.entry.kind === "group"
                        readonly property var processData: table.rows[slot.entry.index] || table.emptyRow
                        readonly property var groupData: table.groups[slot.entry.index] || table.emptyGroup

                        width: list.width
                        height: slot.isGroup ? CelestinaTheme.rowHeight : CelestinaTheme.controlHeightSm

                        ProcessRow {
                            width: parent.width
                            height: parent.height
                            visible: !slot.isGroup
                            enabled: !slot.isGroup
                            columns: table.columns
                            name: slot.processData.name
                            user: slot.processData.user
                            pid: String(slot.processData.pid)
                            cpu: table.percentText(slot.processData.cpu)
                            memory: table.bytesText(slot.processData.memory * 1024)
                            read: table.rateText(slot.processData.read)
                            write: table.rateText(slot.processData.write)
                            actionable: slot.processData.actionable
                            selected: slot.processData.pid === table.selectedPid
                            nested: table.grouped
                            onClicked: table.selectedPid = slot.processData.pid
                        }

                        ApplicationRow {
                            width: parent.width
                            height: parent.height
                            visible: slot.isGroup
                            enabled: slot.isGroup
                            appName: slot.groupData.name
                            iconName: slot.groupData.icon
                            count: slot.groupData.count
                            cpu: table.percentText(slot.groupData.cpu)
                            memory: table.bytesText(slot.groupData.memory * 1024)
                            expanded: !table.collapsed[slot.groupData.id]
                            onClicked: table.toggleGroup(slot.groupData.id)
                        }
                    }
                }

                // The bar reports on the list from beside it: it is not a
                // `ScrollBar` attachment, and a child of the list itself
                // would scroll away with the rows.
                CelestinaScrollBar {
                    surface: list
                    anchors.right: list.right
                    anchors.top: list.top
                    anchors.bottom: list.bottom
                }
            }
        }
    }

    KillDialog {
        id: killDialog
        anchors.fill: parent
        backdrop: table.backdrop
        onConfirmed: function(pid) { table.processes.kill(pid) }
    }
}
