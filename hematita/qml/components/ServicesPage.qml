pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.hematita 1.0

// Servicios: every unit of both managers in one list, with the search, the two
// scope toggles and the three actions on the bar. Every word is composed here
// from tokens; a unit's name and description are systemd's own data, shown raw.
Item {
    id: page

    required property HematitaServices services
    // What the confirmation blurs beneath itself.
    required property Item backdrop

    // The selection is a unit, which is its name and its manager together: the
    // same name can exist on both buses.
    property string selectedKey: ""
    property var rows: []

    readonly property var emptyRow: ({ name: "", description: "", scope: "", active: "",
                                       sub: "", kind: "", actionable: false, key: "" })

    function keyOf(name, scope) {
        return scope + "/" + name
    }

    // The one place the typed word reaches the hub, from the clock or from
    // Return.
    function applyFilter() {
        page.services.filterText = search.text
        page.services.refresh()
    }

    function weave() {
        const published = page.services
        // Defensive: a short column would mean a publication error, and fewer
        // rows are better than rows with undefined fields.
        const count = Math.min(published.unitNames.length, published.unitDescriptions.length,
                               published.unitScopes.length, published.unitActives.length,
                               published.unitSubs.length, published.unitKinds.length,
                               published.unitActionable.length)
        const woven = []
        for (let index = 0; index < count; ++index)
            woven.push({ name: published.unitNames[index],
                         description: published.unitDescriptions[index],
                         scope: published.unitScopes[index],
                         active: published.unitActives[index],
                         sub: published.unitSubs[index],
                         kind: published.unitKinds[index],
                         actionable: published.unitActionable[index] === 1,
                         key: page.keyOf(published.unitNames[index], published.unitScopes[index]) })
        // The assignment is made under `anchoring`, so the list's own
        // `currentIndexChanged` — which a shorter model fires before the
        // re-anchor runs — cannot be read as the person moving the selection
        // and clear an answer they have not seen yet.
        page.anchoring = true
        page.rows = woven
        page.anchoring = false
        page.anchorCursor()
    }

    // The rows are rebuilt on every revision, and a rebuilt list keeps
    // whatever index it had — which is a different unit. This is the one place
    // that puts the cursor back on the selection, and the one place that lets
    // go of a selection that is no longer listed. It does not clear the last
    // action's answer: a unit that left the filter is not the person moving on.
    property bool anchoring: false
    function anchorCursor() {
        if (page.anchoring)
            return
        page.anchoring = true
        const index = page.indexOfKey(page.selectedKey)
        if (index >= 0)
            list.currentIndex = index
        else if (page.selectedKey.length > 0)
            page.selectedKey = ""
        page.anchoring = false
    }

    function indexOfKey(key) {
        for (let index = 0; index < page.rows.length; ++index)
            if (page.rows[index].key === key)
                return index
        return -1
    }

    function selectedRow() {
        const index = page.indexOfKey(page.selectedKey)
        return index >= 0 ? page.rows[index] : null
    }

    function actionVerb(kind) {
        switch (kind) {
        case "start": return qsTr("Iniciar")
        case "stop": return qsTr("Detener")
        case "restart": return qsTr("Reiniciar")
        }
        return ""
    }

    function outcomeText() {
        const published = page.services
        if (published.actionOutcome === "")
            return ""
        const verb = page.actionVerb(published.actionKind)
        switch (published.actionOutcome) {
        case "done":
            return qsTr("%1: hecho").arg(verb)
        case "pending":
            return page.services.actionScope === "system"
                   ? qsTr("%1: esperando la autorización").arg(verb)
                   : qsTr("%1: en curso").arg(verb)
        case "no-agent":
            return qsTr("%1: esta sesión no tiene agente de autenticación; la acción sobre unidades del sistema necesita uno")
                     .arg(verb)
        case "denied":
            return qsTr("%1: autorización denegada o cancelada").arg(verb)
        case "failed":
            return qsTr("%1: systemd rechazó la acción").arg(verb)
        case "refused":
            return qsTr("%1: esta unidad no admite acciones").arg(verb)
        }
        return ""
    }

    // Whether the manager the last action was asked of is answering. A bus
    // that is not there is the first thing the page says — unless there is an
    // answer about an action on a bus that *is* there, which is what the
    // person is waiting to read.
    function actionBusAvailable() {
        if (page.services.actionScope === "system")
            return page.services.systemAvailable
        if (page.services.actionScope === "user")
            return page.services.userAvailable
        return false
    }

    function noteText() {
        const outcome = page.outcomeText()
        if (outcome.length > 0 && page.actionBusAvailable())
            return outcome
        if (!page.services.systemAvailable)
            return qsTr("No se pudo leer el bus del sistema")
        if (!page.services.userAvailable)
            return qsTr("No se pudo leer el bus de sesión")
        return outcome
    }

    function act(kind) {
        const row = page.selectedRow()
        if (row === null || !row.actionable)
            return
        if (kind === "start") {
            page.services.startUnit(row.name, row.scope)
            return
        }
        confirm.ask(kind === "stop" ? qsTr("¿Detener «%1»?").arg(row.name)
                                    : qsTr("¿Reiniciar «%1»?").arg(row.name),
                    kind === "stop" ? qsTr("Detener") : qsTr("Reiniciar"),
                    { kind: kind, name: row.name, scope: row.scope })
    }

    // A click writes `selectedKey` and the arrow keys write `currentIndex`;
    // each writes the other back, so the cursor and the selection are one
    // thing however they were moved.
    onSelectedKeyChanged: {
        if (page.anchoring)
            return
        const index = page.indexOfKey(page.selectedKey)
        if (index >= 0)
            list.currentIndex = index
        // Only a selection the person made is a new question, so only that one
        // clears the last action's answer.
        page.services.clearAction()
    }

    Connections {
        target: page.services
        function onRevisionChanged() { if (page.visible) page.weave() }
    }

    onVisibleChanged: if (page.visible) page.weave()

    Component.onCompleted: {
        // The hub owns the defaults; the glyphs are set from it once here.
        systemToggle.checked = page.services.showSystem
        userToggle.checked = page.services.showUser
        servicesToggle.checked = page.services.servicesOnly
        page.weave()
    }

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
                placeholderText: qsTr("Buscar por nombre o descripción")
                Accessible.name: search.placeholderText
                // The filter re-sorts every unit, so a typed word does not do
                // it once per letter: the last keystroke of a burst is what
                // reaches the hub. Return says the word is finished.
                onTextChanged: debounce.restart()
                onAccepted: {
                    debounce.stop()
                    page.applyFilter()
                }
            }

            Timer {
                id: debounce
                interval: 150
                onTriggered: page.applyFilter()
            }

            Text {
                text: page.services.shownCount === page.services.totalCount
                      ? qsTr("%1 unidades").arg(page.services.totalCount)
                      : qsTr("%1 de %2 unidades").arg(page.services.shownCount)
                                                 .arg(page.services.totalCount)
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                font.features: CelestinaTheme.fontFeaturesTabular
            }

            Item { Layout.fillWidth: true }

            // The three filters. The glyph is written from the hub once, when
            // the page is built, and the toggle writes the hub back: a
            // checkable button writes its own `checked` on the click, so a
            // standing binding would be replaced on the first press and leave
            // the glyph telling a different story from the list.
            CelestinaCapsule {
                CelestinaIconButton {
                    id: systemToggle

                    iconName: "monitor"
                    helpText: qsTr("Servicios del sistema")
                    role: CelestinaButton.Ghost
                    checkable: true
                    onToggled: {
                        page.services.showSystem = systemToggle.checked
                        page.services.refresh()
                    }
                }

                CelestinaIconButton {
                    id: userToggle

                    iconName: "app-window"
                    helpText: qsTr("Servicios de usuario")
                    role: CelestinaButton.Ghost
                    checkable: true
                    onToggled: {
                        page.services.showUser = userToggle.checked
                        page.services.refresh()
                    }
                }

                CelestinaIconButton {
                    id: servicesToggle

                    iconName: "view-list"
                    helpText: qsTr("Solo servicios")
                    role: CelestinaButton.Ghost
                    checkable: true
                    onToggled: {
                        page.services.servicesOnly = servicesToggle.checked
                        page.services.refresh()
                    }
                }
            }

            CelestinaCapsule {
                CelestinaIconButton {
                    iconName: "media-play"
                    helpText: qsTr("Iniciar")
                    role: CelestinaButton.Ghost
                    enabled: page.selectedRow() !== null && page.selectedRow().actionable
                    onClicked: page.act("start")
                }

                CelestinaIconButton {
                    iconName: "circle-stop"
                    helpText: qsTr("Detener")
                    role: CelestinaButton.Ghost
                    enabled: page.selectedRow() !== null && page.selectedRow().actionable
                    onClicked: page.act("stop")
                }

                CelestinaIconButton {
                    iconName: "view-refresh"
                    helpText: qsTr("Reiniciar")
                    role: CelestinaButton.Ghost
                    enabled: page.selectedRow() !== null && page.selectedRow().actionable
                    onClicked: page.act("restart")
                }
            }
        }

        // ── What the page has to say, if anything ──────────────────────
        Text {
            id: note

            Layout.fillWidth: true
            visible: note.text.length > 0
            text: page.noteText()
            color: (page.services.systemAvailable && page.services.userAvailable
                    && page.services.actionOutcome !== "failed"
                    && page.services.actionOutcome !== "denied")
                   ? CelestinaTheme.textMuted : CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            wrapMode: Text.WordWrap
        }

        // ── Rows ───────────────────────────────────────────────────────
        CelestinaSurface {
            Layout.fillWidth: true
            Layout.fillHeight: true
            role: CelestinaSurface.Panel
            padding: CelestinaTheme.spaceXs

            contentItem: Item {
                // A list, not a column of focusable rows: the page is one Tab
                // stop and the arrows move it unit by unit. The model is the
                // count, so a tick that changes no unit leaves the rows where
                // they are.
                ListView {
                    id: list

                    anchors.fill: parent
                    clip: true
                    model: page.rows.length
                    activeFocusOnTab: true
                    keyNavigationEnabled: true
                    Accessible.role: Accessible.List
                    Accessible.name: qsTr("Servicios")

                    onCurrentIndexChanged: {
                        if (page.anchoring)
                            return
                        if (list.currentIndex >= 0 && list.currentIndex < page.rows.length)
                            page.selectedKey = page.rows[list.currentIndex].key
                    }

                    delegate: ServiceRow {
                        id: unitRow

                        required property int index

                        readonly property var unitData: unitRow.index >= 0
                                                        && unitRow.index < page.rows.length
                                                        ? page.rows[unitRow.index] : page.emptyRow

                        width: list.width
                        unitName: unitRow.unitData.name
                        unitDescription: unitRow.unitData.description
                        scope: unitRow.unitData.scope
                        active: unitRow.unitData.active
                        selected: unitRow.unitData.key.length > 0
                                  && unitRow.unitData.key === page.selectedKey
                        onClicked: page.selectedKey = unitRow.unitData.key
                    }
                }

                // The bar reports on the list from beside it: a child of the
                // list itself would scroll away with the rows.
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
        backdrop: page.backdrop
        onConfirmed: function(payload) {
            if (payload === null)
                return
            if (payload.kind === "stop")
                page.services.stopUnit(payload.name, payload.scope)
            else if (payload.kind === "restart")
                page.services.restartUnit(payload.name, payload.scope)
        }
    }
}
