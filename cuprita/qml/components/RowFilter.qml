pragma ComponentBehavior: Bound

import QtQuick
import QtQml.Models

// One page's view of a list model: only the rows `accepts` keeps, in the
// model's order. The model keeps every row; a page shows the same rows in
// several cards, each through its own filter. The filter runs again whenever
// rows come, go, move or change.
DelegateModel {
    id: filter

    // Whether a row — its roles, read as `row.name` — belongs in this view.
    property var accepts: function(row) { return true }
    // Keep only the first row accepted.
    property bool firstOnly: false
    // The `id` roles of the rows kept, in order.
    property var keptIds: []
    // A pass is scheduled and has not run: the kept rows may be stale, so a
    // page waits for it before saying a card is empty.
    property bool pending: false

    // One pass reads every row's roles (O(rows) per filter, three filters on
    // the Red page); a burst of row signals in one JS turn asks for it many
    // times, so `refilter` only schedules it and `Qt.callLater` runs it once.
    function refilter() {
        filter.pending = true
        Qt.callLater(filter.apply)
    }

    function apply() {
        const ids = []
        for (let i = 0; i < filter.items.count; ++i) {
            const item = filter.items.get(i)
            const keep = (!filter.firstOnly || ids.length === 0) && filter.accepts(item.model)
            item.inKept = keep
            if (keep)
                ids.push(item.model.id)
        }
        filter.keptIds = ids
        filter.pending = false
    }

    groups: [
        DelegateModelGroup { name: "kept" }
    ]
    filterOnGroup: "kept"

    onAcceptsChanged: filter.refilter()
    Component.onCompleted: filter.refilter()

    items.onChanged: filter.refilter()

    // A row whose roles change may enter or leave this view.
    property Connections watcher: Connections {
        target: filter.model
        ignoreUnknownSignals: true
        function onDataChanged() { filter.refilter() }
    }
}
