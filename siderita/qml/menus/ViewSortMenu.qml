import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// ─── ViewSortMenu ───────────────────────────────────────────────────────────
// What the bottom bar's middle icon opens: how the folder is shown, and in what
// order. They are one question — how is this arranged — which is why they are
// one menu behind one glyph rather than two pills. The panel owns the view
// mode, the tab's controller owns the sorting; both arrive as properties and
// this component reaches no id outside itself.
// ──────────────────────────────────────────────────────────────────────────────
GlassContextMenu {
    id: root

    // The tab's controller, injected by the folder view.
    property var controller
    // The panel: owner of the view mode and of persisting it.
    property var panel

    function chooseView(mode) {
        root.panel.viewMode = mode
        root.panel.persist()
    }

    CelestinaSectionLabel {
        text: qsTr("VISTA")
    }

    GlassMenuItem {
        text: qsTr("Cuadrícula")
        icon.name: "view-grid"
        icon.source: CelestinaTheme.fallbackIcon("view-grid")
        choice: true
        current: root.panel.viewMode === "grid"
        onTriggered: root.chooseView("grid")
    }

    GlassMenuItem {
        text: qsTr("Lista")
        icon.name: "view-list"
        icon.source: CelestinaTheme.fallbackIcon("view-list")
        choice: true
        current: root.panel.viewMode === "list"
        onTriggered: root.chooseView("list")
    }

    GlassMenuItem {
        text: qsTr("Detalles")
        icon.name: "view-details"
        icon.source: CelestinaTheme.fallbackIcon("view-details")
        choice: true
        current: root.panel.viewMode === "details"
        onTriggered: root.chooseView("details")
    }

    CelestinaSectionLabel {
        text: qsTr("ORDENAR POR")
    }

    GlassMenuItem {
        text: qsTr("Nombre")
        icon.name: "type"
        icon.source: CelestinaTheme.fallbackIcon("type")
        choice: true
        current: root.controller.sortField === 0
        onTriggered: root.controller.changeSortField(0)
    }

    GlassMenuItem {
        text: qsTr("Tamaño")
        icon.name: "hard-drive"
        icon.source: CelestinaTheme.fallbackIcon("hard-drive")
        choice: true
        current: root.controller.sortField === 1
        onTriggered: root.controller.changeSortField(1)
    }

    GlassMenuItem {
        text: qsTr("Fecha de modificación")
        icon.name: "clock-arrow-up"
        icon.source: CelestinaTheme.fallbackIcon("clock-arrow-up")
        choice: true
        current: root.controller.sortField === 2
        onTriggered: root.controller.changeSortField(2)
    }

    GlassMenuItem {
        text: qsTr("Tipo")
        icon.name: "files"
        icon.source: CelestinaTheme.fallbackIcon("files")
        choice: true
        current: root.controller.sortField === 3
        onTriggered: root.controller.changeSortField(3)
    }

    CelestinaSectionLabel {
        text: qsTr("SENTIDO")
    }

    // The direction loses its permanent arrow on the bar: it is read here, and
    // in details mode the column header still says it.
    GlassMenuItem {
        text: root.controller.sortAscending ? qsTr("Ascendente")
                                            : qsTr("Descendente")
        icon.name: root.controller.sortAscending
                   ? "view-sort-ascending" : "view-sort-descending"
        icon.source: CelestinaTheme.fallbackIcon(
                         root.controller.sortAscending
                         ? "view-sort-ascending" : "view-sort-descending")
        onTriggered: root.controller.toggleSortDirection()
    }
}
