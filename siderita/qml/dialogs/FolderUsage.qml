pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import org.celestina.siderita 1.0

// The occupation of a folder, as Hematita computes it: where the person is
// inside the scanned tree, its totals, the biggest children as a list beside
// a treemap, and two ways out — to that folder in Siderita and to Hematita.
// Presents `usage` (a SideritaUsage); decides nothing.
//
// Keys: the crumbs, the list and the map take Tab stops; the list and the
// map own the arrows (Left and Right are stopped inside the section), Enter
// (drill) and Backspace (up). Space and Escape are left unaccepted by both,
// so they reach the modal that hosts this section. Ctrl+Enter, while
// focus is inside the section, goes to the current entry in Siderita.
FocusScope {
    id: section

    required property var usage
    required property var controller
    // Emitted after the controller was asked to open a folder, so the host
    // modal closes.
    signal navigated()
    // Emitted after Hematita was launched on `path`.
    signal hematitaRequested(string path)

    readonly property bool scanning: section.usage.mode === "scanning"
    readonly property bool analysed: section.usage.mode === "analysed"
    readonly property bool failed: section.usage.mode === "failed"
    property int currentId: -1
    property var usageRows: []
    property var tiles: []
    property bool handoffFailed: false
    readonly property var toneColors: ({
        dir: CelestinaTheme.glyphAccentBlue,
        file: CelestinaTheme.glyphAccentViolet,
        unreadable: CelestinaTheme.textFaint,
        other: CelestinaTheme.textFaint
    })

    function bytesText(bytes) {
        if (bytes >= 1099511627776) return (bytes / 1099511627776).toLocaleString(Qt.locale(), "f", 1) + " TiB"
        if (bytes >= 1073741824) return (bytes / 1073741824).toLocaleString(Qt.locale(), "f", 1) + " GiB"
        if (bytes >= 1048576) return (bytes / 1048576).toLocaleString(Qt.locale(), "f", 1) + " MiB"
        if (bytes >= 1024) return (bytes / 1024).toLocaleString(Qt.locale(), "f", 0) + " KiB"
        return qsTr("%1 B").arg(bytes)
    }

    function countText(count) {
        return Number(count).toLocaleString(Qt.locale(), "f", 0)
    }

    function failureText(token) {
        if (token === "not-a-folder") return qsTr("No es una carpeta")
        if (token === "too-many") return qsTr("Tiene demasiadas entradas para analizarla aquí")
        if (token === "thread") return qsTr("No se pudo empezar el análisis")
        return qsTr("No se puede leer esta carpeta")
    }

    function weave() {
        const published = section.usage
        const count = Math.min(published.rowIds.length, published.rowNames.length,
                               published.rowKinds.length, published.rowAllocated.length,
                               published.rowShares.length, published.rowMerged.length,
                               published.rowUnreadable.length)
        const rows = []
        const byId = {}
        for (let index = 0; index < count; ++index) {
            const merged = published.rowMerged[index]
            const share = published.rowShares[index]
            const kind = published.rowKinds[index]
            const unreadable = published.rowUnreadable[index]
            const row = { id: published.rowIds[index],
                          name: merged > 0 ? qsTr("otros (%1)").arg(merged) : published.rowNames[index],
                          kind: kind,
                          tone: unreadable > 0 ? "unreadable"
                                               : (kind === "dir" || kind === "file" ? kind : "other"),
                          share: share,
                          size: section.bytesText(published.rowAllocated[index]),
                          percent: qsTr("%1 %").arg((share * 100).toLocaleString(Qt.locale(), "f", 1)),
                          detail: unreadable > 0 ? qsTr("%1 no legibles").arg(unreadable) : "" }
            rows.push(row)
            byId[row.id] = row
        }
        const rects = published.tiles
        const tiles = []
        for (let at = 0; at + 4 < rects.length; at += 5) {
            const id = rects[at]
            const row = byId[id]
            tiles.push({ id: id, x: rects[at + 1], y: rects[at + 2], w: rects[at + 3],
                         h: rects[at + 4], name: row ? row.name : "",
                         kind: row ? row.kind : "other", tone: row ? row.tone : "other" })
        }
        section.tiles = tiles
        usageList.reset(function() { section.usageRows = rows })
        section.currentId = rows.length > 0 ? rows[0].id : -1
    }

    // A folder goes to itself; a file to the folder that holds it.
    function goTo(id) {
        let kind = ""
        for (let index = 0; index < section.usageRows.length; ++index) {
            if (section.usageRows[index].id === id)
                kind = section.usageRows[index].kind
        }
        const path = kind === "dir" ? section.usage.pathOf(id) : section.usage.currentPath
        if (path.length === 0)
            return
        section.controller.openLocation(path)
        section.navigated()
    }

    function openInHematita() {
        const path = section.usage.currentPath
        section.handoffFailed = !section.usage.openInHematita(path)
        if (!section.handoffFailed)
            section.hematitaRequested(path)
    }

    Component.onCompleted: section.weave()

    Connections {
        target: section.usage
        function onRevisionChanged() {
            section.handoffFailed = false
            section.weave()
        }
    }

    // A Shortcut rather than a key handler: the list and the map accept
    // Return whatever the modifiers, so a handler here would never see it.
    Shortcut {
        sequences: ["Ctrl+Return", "Ctrl+Enter"]
        enabled: section.visible && section.activeFocus && section.analysed
                 && section.currentId >= 0
        onActivated: section.goTo(section.currentId)
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: CelestinaTheme.spaceSm

        // Crumbs: one ghost button per folder from the scanned root, one Tab
        // stop each; a crumb jumps there in one step.
        Item {
            Layout.fillWidth: true
            implicitHeight: crumbRow.implicitHeight
            clip: true
            visible: section.usage.crumbs.length > 0

            Row {
                id: crumbRow
                spacing: CelestinaTheme.spaceXs
                Accessible.role: Accessible.ToolBar
                Accessible.name: qsTr("Ruta")

                Repeater {
                    model: section.usage.crumbs.length

                    delegate: Row {
                        id: crumb

                        required property int index

                        spacing: CelestinaTheme.spaceXs

                        CelestinaIcon {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: crumb.index > 0
                            width: CelestinaTheme.iconSm
                            height: width
                            name: "chevron-right"
                            tone: CelestinaIcon.Secondary
                        }

                        CelestinaButton {
                            objectName: "crumb" + crumb.index
                            role: CelestinaButton.Ghost
                            text: section.usage.crumbs[crumb.index]
                            helpText: text
                            font.weight: crumb.index === section.usage.crumbs.length - 1
                                         ? CelestinaTheme.weightDemiBold : CelestinaTheme.weightRegular
                            onClicked: section.usage.upTo(crumb.index)
                        }
                    }
                }
            }
        }

        Text {
            objectName: "usageTotals"
            Layout.fillWidth: true
            textFormat: Text.PlainText
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideRight
            Accessible.role: Accessible.StaticText
            Accessible.name: text
            text: section.scanning
                  ? qsTr("Calculando… %1 entradas · %2")
                        .arg(section.countText(section.usage.progressEntries))
                        .arg(section.bytesText(section.usage.progressBytes))
                  : section.failed ? section.failureText(section.usage.failure)
                  : section.analysed
                    ? qsTr("%1 · %2 archivos · %3 carpetas · %4 no legibles · %5 en otros dispositivos")
                          .arg(section.bytesText(section.usage.totalBytes))
                          .arg(section.countText(section.usage.filesBelow))
                          .arg(section.countText(section.usage.foldersBelow))
                          .arg(section.countText(section.usage.unreadableBelow))
                          .arg(section.countText(section.usage.otherDevices))
                    : ""
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: CelestinaTheme.spaceSm
            visible: !section.failed

            // Left and Right are the map's; in the list they would bubble to
            // the host modal, which steps to another entry and drops the
            // drill. Inside the section they stop here.
            Keys.onLeftPressed: function(event) { event.accepted = true }
            Keys.onRightPressed: function(event) { event.accepted = true }

            CelestinaUsageList {
                id: usageList
                objectName: "usageList"
                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.preferredWidth: 42
                Layout.horizontalStretchFactor: 42
                usageRows: section.usageRows
                toneColors: section.toneColors
                emptyText: section.scanning ? qsTr("Calculando…") : qsTr("Carpeta vacía")
                onChosen: function(id) { section.currentId = id }
                onEntered: function(id) { section.usage.enter(id) }
                onUpRequested: section.usage.up()
            }

            CelestinaTreemap {
                objectName: "usageMap"
                Layout.fillHeight: true
                Layout.fillWidth: true
                Layout.preferredWidth: 58
                Layout.horizontalStretchFactor: 58
                tiles: section.tiles
                toneColors: section.toneColors
                currentId: section.currentId
                currentName: section.usage.crumbs.length > 0
                             ? section.usage.crumbs[section.usage.crumbs.length - 1] : ""
                onChosen: function(id) {
                    section.currentId = id
                    usageList.follow(id)
                }
                onEntered: function(id) { section.usage.enter(id) }
                onUpRequested: section.usage.up()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: CelestinaTheme.spaceSm

            Text {
                Layout.fillWidth: true
                visible: section.handoffFailed
                textFormat: Text.PlainText
                text: qsTr("No se pudo abrir Hematita")
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                elide: Text.ElideRight
                Accessible.role: Accessible.AlertMessage
                Accessible.name: text
            }

            Item {
                Layout.fillWidth: true
                visible: !section.handoffFailed
            }

            CelestinaButton {
                objectName: "goToFolderButton"
                text: qsTr("Ir a la carpeta")
                role: CelestinaButton.Ghost
                enabled: section.analysed && section.currentId >= 0
                onClicked: section.goTo(section.currentId)
            }

            CelestinaButton {
                objectName: "hematitaButton"
                text: qsTr("Abrir en Hematita")
                role: CelestinaButton.Tonal
                enabled: section.usage.root.length > 0
                onClicked: section.openInHematita()
            }
        }
    }
}
