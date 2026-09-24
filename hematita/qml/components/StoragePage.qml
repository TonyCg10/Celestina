pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.hematita 1.0

// Almacenamiento: the mounted locations with how full each is, the folder
// being browsed inside one of them, and — once scanned — what fills it, as a
// size list beside a treemap, with the duplicate and empty-folder filters.
// The bar carries where the person is, the filters and the actions on the
// selection: open in Siderita, trash (asked first for more than one entry),
// delete permanently (always asked first), details. Every word is composed here from
// tokens; names and paths are the filesystem's own, shown raw. QML names
// browsed rows by index and analysed ones by node id, and never hands a path
// back.
Item {
    id: page

    required property HematitaAnalysis analysis
    // What a confirmation blurs beneath itself.
    required property Item backdrop

    property var locationRows: []
    property var folderRows: []
    // The folder the rows were last woven for, so a refresh of the same
    // folder keeps the person's place and a new folder starts at its top.
    property string folderKey: ""

    property var usageRows: []
    property var tiles: []
    property var duplicateRows: []
    // The analysed folder and filters the rows were last woven for.
    property string usageKey: ""
    // The analysed entry under the cursor, shared by the list and the map.
    property int currentId: -1
    property bool detailsShown: false

    readonly property bool browsing: page.analysis.mode === "browsing"
    readonly property bool scanning: page.analysis.mode === "scanning"
    readonly property bool analysed: page.analysis.mode === "analysed"
    readonly property bool filtered: page.analysis.showDuplicates || page.analysis.showEmpty
    readonly property bool canAct: page.analysed && page.analysis.selectedIds.length > 0
                                   && !page.analysis.busy
    readonly property string outcomeText: {
        const verbs = { open: qsTr("Abrir"), trash: qsTr("Papelera"), delete: qsTr("Borrar") }
        const verb = verbs[page.analysis.actionKind] || ""
        switch (page.analysis.actionOutcome) {
        case "done":
            return qsTr("%1: hecho (%2)").arg(verb).arg(page.bytesText(page.analysis.actionBytes))
        case "partial":
            return qsTr("%1: %2 de %3").arg(verb).arg(page.analysis.actionDone)
                                       .arg(page.analysis.actionTotal)
        case "failed":
            return qsTr("%1: no se pudo").arg(verb)
        case "refused":
            return qsTr("%1: rechazado").arg(verb)
        case "cancelled":
            return qsTr("%1: cancelado tras %2 de %3").arg(verb).arg(page.analysis.actionDone)
                                                      .arg(page.analysis.actionTotal)
        }
        return ""
    }
    // The colour each kind of entry paints, in the list's bars and the map.
    readonly property var toneColors: ({
        dir: CelestinaTheme.glyphAccentBlue,
        file: CelestinaTheme.glyphAccentViolet,
        duplicate: CelestinaTheme.glyphAccentCoral,
        empty: CelestinaTheme.glyphAccentAmber,
        unreadable: CelestinaTheme.textFaint,
        other: CelestinaTheme.textFaint
    })
    readonly property string progressText: qsTr("%1 archivos · %2")
                                               .arg(page.analysis.progressFiles.toLocaleString(Qt.locale(), "f", 0))
                                               .arg(page.bytesText(page.analysis.progressBytes))
    readonly property var currentRow: {
        for (let index = 0; index < page.usageRows.length; ++index) {
            if (page.usageRows[index].id === page.currentId)
                return page.usageRows[index]
        }
        return null
    }
    readonly property string currentFolderName: {
        const names = page.analysis.crumbNames
        return names.length > 0 ? names[names.length - 1] : ""
    }

    function bytesText(bytes) {
        if (bytes >= 1099511627776) return (bytes / 1099511627776).toLocaleString(Qt.locale(), "f", 1) + " TiB"
        if (bytes >= 1073741824) return (bytes / 1073741824).toLocaleString(Qt.locale(), "f", 1) + " GiB"
        if (bytes >= 1048576) return (bytes / 1048576).toLocaleString(Qt.locale(), "f", 1) + " MiB"
        if (bytes >= 1024) return (bytes / 1024).toLocaleString(Qt.locale(), "f", 0) + " KiB"
        return qsTr("%1 B").arg(bytes)
    }

    function toneOf(kind, duplicate, empty, unreadable) {
        if (unreadable === 1) return "unreadable"
        if (duplicate === 1) return "duplicate"
        if (empty === 1) return "empty"
        if (kind === "dir" || kind === "file") return kind
        return "other"
    }

    function weaveAnalysed() {
        const published = page.analysis
        const count = Math.min(published.entryIds.length, published.entryNames.length,
                               published.entryKinds.length, published.entryAllocated.length,
                               published.entryApparent.length, published.entryShares.length,
                               published.entryFilesBelow.length, published.entryEmpty.length,
                               published.entryDuplicate.length,
                               published.entryUnreadable.length)
        const rows = []
        const byId = {}
        for (let index = 0; index < count; ++index) {
            const share = published.entryShares[index]
            const row = { id: published.entryIds[index],
                          name: published.entryNames[index],
                          kind: published.entryKinds[index],
                          tone: page.toneOf(published.entryKinds[index],
                                            published.entryDuplicate[index],
                                            published.entryEmpty[index],
                                            published.entryUnreadable[index]),
                          share: share,
                          size: page.bytesText(published.entryAllocated[index]),
                          percent: qsTr("%1 %").arg((share * 100).toLocaleString(Qt.locale(), "f", 1)),
                          apparent: page.bytesText(published.entryApparent[index]),
                          copies: index < published.entryCopies.length
                                  ? published.entryCopies[index] : 0,
                          files: published.entryFilesBelow[index].toLocaleString(Qt.locale(), "f", 0) }
            rows.push(row)
            byId[row.id] = row
        }

        const rects = published.treemapRects
        const tiles = []
        for (let at = 0; at + 4 < rects.length; at += 5) {
            const id = rects[at]
            const row = byId[id]
            tiles.push({ id: id, x: rects[at + 1], y: rects[at + 2], w: rects[at + 3],
                         h: rects[at + 4], name: row ? row.name : "",
                         tone: row ? row.tone : "other",
                         matches: id < 0 ? !page.filtered : row !== undefined })
        }
        page.tiles = tiles

        const groups = Math.min(published.groupSizes.length, published.groupCounts.length,
                                published.groupVerified.length,
                                published.groupUnreadable.length)
        const members = Math.min(published.memberGroups.length, published.memberIds.length,
                                 published.memberNames.length, published.memberPaths.length)
        const duplicates = []
        let member = 0
        for (let group = 0; group < groups; ++group) {
            duplicates.push({ header: true, group: group, count: published.groupCounts[group],
                              size: page.bytesText(published.groupSizes[group]),
                              verified: published.groupVerified[group] === 1,
                              unreadable: published.groupUnreadable[group] === 1,
                              id: -1, name: "", path: "" })
            while (member < members && published.memberGroups[member] === group) {
                duplicates.push({ header: false, group: group, count: 0, size: "",
                                  verified: false, unreadable: false,
                                  id: published.memberIds[member],
                                  name: published.memberNames[member],
                                  path: published.memberPaths[member] })
                ++member
            }
        }
        duplicateList.keepViewport(function() { page.duplicateRows = duplicates })

        const key = published.crumbPaths.join("\n") + "|" + published.showDuplicates + "|"
                    + published.showEmpty
        if (key !== page.usageKey || page.usageRows.length === 0) {
            page.usageKey = key
            usageList.reset(function() { page.usageRows = rows })
        } else {
            usageList.keepViewport(function() { page.usageRows = rows })
        }
    }

    // The entry under the cursor, as the details card reads it.
    function detailsOf(row) {
        if (row === null)
            return { name: "", path: "", allocated: "", apparent: "", files: "", copies: [] }
        const published = page.analysis
        const folder = published.crumbPaths.length > 0
                       ? published.crumbPaths[published.crumbPaths.length - 1] : ""
        const copies = []
        const at = published.memberIds.indexOf(row.id)
        if (at >= 0 && published.groupVerified[published.memberGroups[at]] === 1) {
            for (let index = 0; index < published.memberIds.length; ++index) {
                if (index !== at && published.memberGroups[index] === published.memberGroups[at])
                    copies.push(published.memberPaths[index])
            }
        }
        return { name: row.name, path: (folder === "/" ? "" : folder) + "/" + row.name,
                 allocated: row.size, apparent: row.apparent, files: row.files, copies: copies }
    }

    function weave() {
        const published = page.analysis
        const locationCount = Math.min(published.locationNames.length, published.locationPaths.length,
                                       published.locationKinds.length, published.locationUsed.length,
                                       published.locationTotal.length,
                                       published.locationReadable.length)
        const locations = []
        for (let index = 0; index < locationCount; ++index) {
            const used = published.locationUsed[index]
            const total = published.locationTotal[index]
            locations.push({ kind: published.locationKinds[index],
                             name: published.locationNames[index],
                             path: published.locationPaths[index],
                             share: total > 0 ? used / total : 0,
                             usage: total > 0
                                    ? qsTr("%1 de %2").arg(page.bytesText(used))
                                                      .arg(page.bytesText(total))
                                    : "",
                             readable: published.locationReadable[index] === 1 })
        }
        locationList.keepViewport(function() { page.locationRows = locations })

        const folderCount = Math.min(published.browseNames.length, published.browseKinds.length,
                                     published.browseApparent.length)
        const folder = []
        for (let index = 0; index < folderCount; ++index)
            folder.push({ name: published.browseNames[index],
                          kind: published.browseKinds[index],
                          size: page.bytesText(published.browseApparent[index]) })
        const key = published.crumbPaths.join("\n")
        if (key !== page.folderKey || page.folderRows.length === 0) {
            page.folderKey = key
            folderList.reset(function() { page.folderRows = folder })
        } else {
            folderList.keepViewport(function() { page.folderRows = folder })
        }
        page.weaveAnalysed()
    }

    // Entering or leaving swaps the list on show; the keyboard follows it, so
    // Enter and Backspace keep working without another Tab.
    function focusBody() {
        if (page.analysed && page.analysis.showDuplicates)
            duplicateList.takeFocus()
        else if (page.analysed)
            usageList.takeFocus()
        else if (page.browsing)
            folderList.takeFocus()
        else
            locationList.takeFocus()
    }

    // Delete and the trash button: one entry goes at once, more are asked.
    function requestTrash() {
        if (!page.canAct)
            return
        const count = page.analysis.selectedCount
        if (count > 1)
            confirm.ask(qsTr("¿Enviar %1 elementos (%2) a la papelera?")
                            .arg(count).arg(page.bytesText(page.analysis.selectedBytes)),
                        qsTr("Papelera"),
                        { kind: "trash", revision: page.analysis.selectionRevision })
        else
            page.analysis.trashSelected(page.analysis.selectionRevision)
    }

    function requestDelete() {
        if (!page.canAct)
            return
        confirm.ask(qsTr("¿Borrar definitivamente %1 elementos (%2)? No se podrán recuperar.")
                        .arg(page.analysis.selectedCount)
                        .arg(page.bytesText(page.analysis.selectedBytes)),
                    qsTr("Borrar"),
                    { kind: "delete", revision: page.analysis.selectionRevision })
    }

    Connections {
        target: page.analysis
        function onRevisionChanged() { if (page.visible) page.weave() }
    }

    onVisibleChanged: if (page.visible) page.weave()

    Component.onCompleted: page.weave()

    ColumnLayout {
        anchors.fill: parent
        spacing: CelestinaTheme.spaceSm

        // ── Bar ────────────────────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            spacing: CelestinaTheme.spaceSm

            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: crumbs.implicitHeight
                clip: true

                PathCrumbs {
                    id: crumbs

                    anchors.verticalCenter: parent.verticalCenter
                    names: page.analysis.crumbNames
                    onChosen: function(index) { page.analysis.enterCrumb(index) }
                }
            }

            // The filters work on a scan's result.
            CelestinaCapsule {
                spacing: CelestinaTheme.spaceXs
                inset: CelestinaTheme.spaceXs

                CelestinaIconButton {
                    iconName: "copy"
                    helpText: qsTr("Duplicados")
                    role: CelestinaButton.Ghost
                    checkable: true
                    checked: page.analysis.showDuplicates
                    enabled: page.analysed
                    onToggled: page.analysis.setFilters(checked, page.analysis.showEmpty)
                }

                CelestinaIconButton {
                    iconName: "folder"
                    helpText: qsTr("Vacías")
                    role: CelestinaButton.Ghost
                    checkable: true
                    checked: page.analysis.showEmpty
                    enabled: page.analysed
                    onToggled: page.analysis.setFilters(page.analysis.showDuplicates, checked)
                }
            }

            // Scanning works from a browsed folder; the other actions work on
            // a selection, which S1-D brings.
            CelestinaCapsule {
                spacing: CelestinaTheme.spaceXs
                inset: CelestinaTheme.spaceXs

                CelestinaIconButton {
                    iconName: page.analysed ? "view-refresh" : "gauge"
                    helpText: page.analysed ? qsTr("Volver a escanear") : qsTr("Escanear aquí")
                    role: CelestinaButton.Ghost
                    enabled: page.browsing || page.analysed
                    onClicked: page.analysis.scanHere()
                }

                CelestinaIconButton {
                    iconName: "folder-open"
                    helpText: qsTr("Abrir en Siderita")
                    role: CelestinaButton.Ghost
                    enabled: page.canAct
                    onClicked: page.analysis.openSelected()
                }

                CelestinaIconButton {
                    iconName: "user-trash"
                    helpText: qsTr("Papelera")
                    role: CelestinaButton.Ghost
                    enabled: page.canAct
                    onClicked: page.requestTrash()
                }

                CelestinaIconButton {
                    iconName: "x"
                    helpText: qsTr("Borrar definitivamente")
                    role: CelestinaButton.Ghost
                    enabled: page.canAct
                    onClicked: page.requestDelete()
                }

                // Stops a running trash or deletion; what it already removed stays
                // removed and leaves the lists.
                CelestinaIconButton {
                    visible: page.analysis.actionRunning
                    iconName: "x"
                    helpText: qsTr("Cancelar")
                    role: CelestinaButton.Ghost
                    onClicked: page.analysis.cancelAction()
                }

                CelestinaIconButton {
                    iconName: "info"
                    helpText: qsTr("Detalles")
                    role: CelestinaButton.Ghost
                    checkable: true
                    checked: page.detailsShown
                    enabled: page.analysed
                    onToggled: page.detailsShown = checked
                }
            }

            CelestinaIconButton {
                visible: !page.analysed && !page.scanning
                iconName: "view-refresh"
                helpText: qsTr("Actualizar")
                role: CelestinaButton.Ghost
                onClicked: page.analysis.refresh()
            }
        }

        // ── What the page has to say, if anything ──────────────────────
        Text {
            Layout.fillWidth: true
            visible: page.analysis.startFailed
            text: qsTr("No se pudo iniciar la lectura del disco")
            color: CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            wrapMode: Text.WordWrap
        }

        Text {
            Layout.fillWidth: true
            visible: page.browsing && page.analysis.actionOutcome === "failed"
            text: qsTr("No se pudo analizar esta carpeta")
            color: CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            wrapMode: Text.WordWrap
        }

        Text {
            Layout.fillWidth: true
            visible: page.analysed && page.outcomeText.length > 0
            text: page.outcomeText
            color: page.analysis.actionOutcome === "done" ? CelestinaTheme.textMuted
                                                          : CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            wrapMode: Text.WordWrap
        }

        // ── The body, by mode ──────────────────────────────────────────
        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: page.scanning || page.analysed ? 2 : (page.browsing ? 1 : 0)

            LocationList {
                id: locationList

                locationRows: page.locationRows
                onEntered: function(index) {
                    page.analysis.enter(index)
                    page.focusBody()
                }
            }

            FolderList {
                id: folderList

                folderRows: page.folderRows
                failed: page.analysis.browseFailed
                loading: page.analysis.busy
                onEntered: function(index) {
                    page.analysis.enter(index)
                    page.focusBody()
                }
                onUpRequested: {
                    page.analysis.up()
                    page.focusBody()
                }
            }

            // The scan: its progress in both panels while it runs, replaced in
            // place by the list and the map when it lands.
            RowLayout {
                spacing: CelestinaTheme.spaceSm

                // The list takes 0.42 of the width through stretch factors: a
                // width read from the parent would make the layout arrange
                // itself again from inside its own arrangement.
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.preferredWidth: 42
                    Layout.horizontalStretchFactor: 42
                    spacing: CelestinaTheme.spaceSm

                    StackLayout {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        currentIndex: page.scanning ? 0 : (page.analysis.showDuplicates ? 2 : 1)

                        CelestinaSurface {
                            role: CelestinaSurface.Panel
                            contentItem: ScanProgress {
                                counts: page.progressText
                                path: page.analysis.progressPath
                                onCancelRequested: page.analysis.cancel()
                            }
                        }

                        UsageList {
                            id: usageList

                            usageRows: page.usageRows
                            selectedIds: page.analysis.selectedIds
                            toneColors: page.toneColors
                            filtered: page.filtered
                            onChosen: function(id) { page.currentId = id }
                            onToggled: function(id) { page.analysis.toggleSelected(id) }
                            onEntered: function(id) { page.analysis.enterId(id) }
                            onUpRequested: {
                                page.analysis.up()
                                page.focusBody()
                            }
                            onTrashRequested: page.requestTrash()
                        }

                        DuplicateList {
                            id: duplicateList

                            duplicateRows: page.duplicateRows
                            selectedIds: page.analysis.selectedIds
                            checking: page.analysis.busy
                            hiddenGroups: page.analysis.hiddenGroupCount
                            onConfirmRequested: page.analysis.confirmDuplicates()
                            onAllButOneRequested: function(group) {
                                page.analysis.selectAllButOne(group)
                            }
                            onToggled: function(id) { page.analysis.toggleSelected(id) }
                            onTrashRequested: page.requestTrash()
                        }
                    }

                    DetailsCard {
                        Layout.fillWidth: true
                        visible: page.analysed && page.detailsShown && page.currentRow !== null
                        details: page.detailsOf(page.currentRow)
                    }
                }

                StackLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.preferredWidth: 58
                    Layout.horizontalStretchFactor: 58
                    currentIndex: page.scanning ? 0 : 1

                    CelestinaSurface {
                        role: CelestinaSurface.Panel
                        contentItem: ScanProgress {
                            counts: page.progressText
                            path: page.analysis.progressPath
                            onCancelRequested: page.analysis.cancel()
                        }
                    }

                    Treemap {
                        tiles: page.tiles
                        selectedIds: page.analysis.selectedIds
                        toneColors: page.toneColors
                        currentId: page.currentId
                        currentName: page.currentFolderName
                        onChosen: function(id) {
                            page.currentId = id
                            usageList.follow(id)
                        }
                        onToggled: function(id) { page.analysis.toggleSelected(id) }
                        onEntered: function(id) { page.analysis.enterId(id) }
                        onUpRequested: page.analysis.up()
                        onTrashRequested: page.requestTrash()
                    }
                }
            }
        }
    }

    ConfirmDialog {
        id: confirm

        anchors.fill: parent
        backdrop: page.backdrop
        onConfirmed: function(payload) {
            if (payload === null)
                return
            // The selection revision the person was asked under; the hub
            // refuses the action when the selection changed since, even to
            // another set of the same count.
            if (payload.kind === "trash")
                page.analysis.trashSelected(payload.revision)
            else if (payload.kind === "delete")
                page.analysis.deleteSelected(payload.revision)
        }
    }
}
