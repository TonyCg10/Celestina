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

    // Below this the rates do not fit beside everything else, so they are the
    // columns that go: the name, the owner, the PID, the CPU and the memory
    // are what the table is for.
    readonly property int ratesWidth: 760
    readonly property bool ratesShown: table.width >= table.ratesWidth
    // Every column but the name has a width of its own; the name takes what
    // is left, and never less than its floor, so the sum can exceed the
    // window only when the window is narrower than the floor plus the rest.
    // The rows sit inside the surface's padding; the header is given the same
    // inset below, so the name column's share has to give that padding back on
    // both sides or the titles would sit one inset off their values.
    readonly property int fixedTotal: 90 + 80 + 80 + 110 + (table.ratesShown ? 220 : 0)
                                      + 2 * CelestinaTheme.spaceXs

    readonly property var columns: [
        { field: "name", title: qsTr("Nombre"), width: Math.max(160, table.width - table.fixedTotal), numeric: false, shown: true },
        { field: "user", title: qsTr("Usuario"), width: 90, numeric: false, shown: true },
        { field: "pid", title: qsTr("PID"), width: 80, numeric: true, shown: true },
        { field: "cpu", title: qsTr("CPU"), width: 80, numeric: true, shown: true },
        { field: "memory", title: qsTr("Memoria"), width: 110, numeric: true, shown: true },
        { field: "read", title: qsTr("Lectura"), width: 110, numeric: true, shown: table.ratesShown },
        { field: "write", title: qsTr("Escritura"), width: 110, numeric: true, shown: table.ratesShown }
    ]

    // The one place the typed word reaches the hub, from the clock or from
    // Return.
    function applyFilter() {
        table.processes.filterText = search.text
        table.processes.refresh()
    }

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
        // The assignment is made under `anchoring`, so the list's own
        // `currentIndexChanged` — which a shorter model fires before the
        // re-anchor runs — cannot be read as the person moving the selection
        // and clear an answer they have not seen yet.
        table.anchoring = true
        table.rows = woven
        table.groups = wovenGroups
        table.entries = table.layout()
        table.anchoring = false
        table.anchorCursor()
    }

    function layout() {
        const out = []
        if (!table.grouped) {
            for (let index = 0; index < table.rows.length; ++index)
                out.push({ kind: "process", index: index })
            return out
        }
        // One pass over the rows into per-application buckets, then one pass
        // over the applications: a nested scan would read every row once per
        // application, which on two thousand processes is the whole table
        // squared for nothing.
        const buckets = []
        for (let group = 0; group < table.groups.length; ++group)
            buckets.push([])
        for (let index = 0; index < table.rows.length; ++index) {
            const group = table.rows[index].group
            if (group >= 0 && group < buckets.length)
                buckets[group].push(index)
        }
        for (let group = 0; group < table.groups.length; ++group) {
            out.push({ kind: "group", index: group })
            if (table.collapsed[table.groups[group].id])
                continue
            for (const index of buckets[group])
                out.push({ kind: "process", index: index })
        }
        return out
    }

    // The entries are rebuilt on every revision and on every fold, and a
    // rebuilt list keeps whatever index it had — which is a different row.
    // This is the one place that puts the cursor back on the selection, and
    // the one place that lets go of a selection that is no longer there.
    property bool anchoring: false
    function anchorCursor() {
        if (table.anchoring)
            return
        table.anchoring = true
        const index = table.entryOfPid(table.selectedPid)
        if (index >= 0)
            list.currentIndex = index
        else if (table.selectedPid >= 0)
            table.selectedPid = -1
        table.anchoring = false
    }

    function toggleGroup(id) {
        const next = Object.assign({}, table.collapsed)
        if (next[id])
            delete next[id]
        else
            next[id] = true
        table.collapsed = next
        table.anchoring = true
        table.entries = table.layout()
        table.anchoring = false
        table.anchorCursor()
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

    // Folds the application under the cursor: `direction` is -1 to collapse,
    // 1 to expand and 0 to toggle. Answers whether it acted, so a key over a
    // process row is left to the list.
    function foldCurrent(direction) {
        if (!table.grouped)
            return false
        const index = list.currentIndex
        if (index < 0 || index >= table.entries.length)
            return false
        const entry = table.entries[index]
        if (entry.kind !== "group" || entry.index < 0 || entry.index >= table.groups.length)
            return false
        const id = table.groups[entry.index].id
        const collapsed = table.collapsed[id] === true
        if (direction < 0 && collapsed)
            return true
        if (direction > 0 && !collapsed)
            return true
        table.toggleGroup(id)
        return true
    }

    function selectedRow() {
        for (let index = 0; index < table.rows.length; ++index)
            if (table.rows[index].pid === table.selectedPid)
                return table.rows[index]
        return null
    }

    // The question the kill asks. A foreign row says so, and says that the
    // answer will be a prompt rather than a dead process.
    function killQuestion(row) {
        if (row === null)
            return ""
        if (!row.actionable)
            return qsTr("¿Matar «%1» (%2)? Pertenece a %3 y pedirá autorización; si el proceso termina mientras esperas, la señal podría alcanzar a otro con el mismo PID.")
                     .arg(row.name).arg(row.pid).arg(row.user)
        return qsTr("¿Matar «%1» (%2)? El proceso no podrá guardar nada.")
                 .arg(row.name).arg(row.pid)
    }

    function outcomeText() {
        const published = table.processes
        if (published.actionOutcome === "")
            return ""
        const verb = published.actionKind === "kill" ? qsTr("Matar") : qsTr("Terminar")
        switch (published.actionOutcome) {
        case "done":
            return qsTr("%1: señal enviada al proceso %2").arg(verb).arg(published.actionPid)
        case "pending":
            return qsTr("%1: esperando la autorización").arg(verb)
        case "no-agent":
            return qsTr("%1: esta sesión no tiene agente de autenticación; actuar sobre un proceso ajeno necesita uno")
                     .arg(verb)
        case "denied":
            return qsTr("%1: autorización denegada o cancelada").arg(verb)
        case "refused":
            return qsTr("%1: el proceso %2 no admite esta acción").arg(verb).arg(published.actionPid)
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
        // A selection released by the re-anchor is not the person moving on:
        // the row simply left the rebuilt list. Only a selection the person
        // made is a new question, so only that one clears the last action's
        // answer — a terminate keeps its sentence until another row is picked.
        if (table.anchoring)
            return
        const index = table.entryOfPid(table.selectedPid)
        if (index >= 0)
            list.currentIndex = index
        table.processes.clearAction()
    }

    // A hidden page costs nothing per tick: the other page of the pair is
    // showing, and this one is rebuilt whole the moment it comes back.
    Connections {
        target: table.processes
        function onRevisionChanged() { if (table.visible) table.weave() }
    }

    onVisibleChanged: if (table.visible) table.weave()

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
                // The filter re-sorts every process, so a typed word does not
                // do it once per letter: the last keystroke of a burst is what
                // reaches the hub, a sixth of a second later. Return says the
                // word is finished and does not wait for the clock.
                onTextChanged: debounce.restart()
                onAccepted: {
                    debounce.stop()
                    table.applyFilter()
                }
            }

            Timer {
                id: debounce
                interval: 150
                onTriggered: table.applyFilter()
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
                // Both actions are offered for any selected row, the
                // person's own or somebody else's: `actionable` only words the
                // bar, and the hub is what decides whether a signal goes
                // straight to the kernel or through an authorisation prompt. A
                // button disabled on a foreign row would make the whole
                // privileged path unreachable.
                CelestinaIconButton {
                    iconName: "circle-stop"
                    helpText: qsTr("Terminar el proceso seleccionado")
                    role: CelestinaButton.Ghost
                    enabled: table.selectedRow() !== null
                    onClicked: table.processes.terminate(table.selectedPid)
                }

                CelestinaIconButton {
                    iconName: "x"
                    helpText: qsTr("Matar el proceso seleccionado")
                    role: CelestinaButton.Ghost
                    enabled: table.selectedRow() !== null
                    onClicked: confirm.ask(table.killQuestion(table.selectedRow()),
                                           qsTr("Matar"), table.selectedPid)
                }
            }
        }

        // ── What the table has to say, if anything ─────────────────────
        Text {
            id: note

            Layout.fillWidth: true
            visible: note.text.length > 0
            // A table that could not be read says so before it says anything
            // about a row in it: the rows are the stale ones.
            text: {
                if (!table.processes.available)
                    return qsTr("No se pudo leer %1").arg(table.processes.reasonPath)
                const row = table.selectedRow()
                // An answer about this very row outranks the standing note
                // about who owns it: the person just asked for something and
                // the answer is what they are waiting to read.
                const outcome = table.outcomeText()
                if (row !== null && table.processes.actionPid === row.pid
                    && table.processes.actionOutcome !== "")
                    return outcome
                if (row !== null && !row.actionable)
                    return qsTr("El proceso %1 pertenece a %2; terminarlo o matarlo pedirá autorización")
                             .arg(row.pid).arg(row.user)
                return outcome
            }
            color: table.processes.available && table.processes.actionOutcome !== "failed"
                   && table.processes.actionOutcome !== "denied"
                   ? CelestinaTheme.textMuted : CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideRight
        }

        // ── Header ─────────────────────────────────────────────────────
        ProcessHeader {
            Layout.fillWidth: true
            Layout.leftMargin: CelestinaTheme.spaceXs
            Layout.rightMargin: CelestinaTheme.spaceXs
            columns: table.columns
            sortField: table.processes.sortField
            sortAscending: table.processes.sortAscending
            onSortRequested: function(field) {
                if (table.processes.sortField === field) {
                    table.processes.sortAscending = !table.processes.sortAscending
                } else {
                    table.processes.sortField = field
                    // Text and identifiers read from the top; rates and
                    // sizes are asked largest-first.
                    table.processes.sortAscending =
                        field === "name" || field === "pid" || field === "user"
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
                        if (table.anchoring)
                            return
                        const pid = table.pidOfEntry(list.currentIndex)
                        if (pid >= 0)
                            table.selectedPid = pid
                    }

                    // ── finding: a group folds from the keyboard too ──
                    // Space and Return toggle the application under the
                    // cursor; Left and Right say which way, so a fold is
                    // reachable without knowing its current state.
                    Keys.onSpacePressed: function(event) { event.accepted = table.foldCurrent(0) }
                    Keys.onReturnPressed: function(event) { event.accepted = table.foldCurrent(0) }
                    Keys.onEnterPressed: function(event) { event.accepted = table.foldCurrent(0) }
                    Keys.onLeftPressed: function(event) { event.accepted = table.foldCurrent(-1) }
                    Keys.onRightPressed: function(event) { event.accepted = table.foldCurrent(1) }

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

    ConfirmDialog {
        id: confirm
        anchors.fill: parent
        backdrop: table.backdrop
        onConfirmed: function(pid) { table.processes.kill(pid) }
    }
}
