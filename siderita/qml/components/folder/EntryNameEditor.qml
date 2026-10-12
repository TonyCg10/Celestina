import QtQuick
import QtQuick.Window
import org.celestina.siderita 1.0

// ─── EntryNameEditor ─────────────────────────────────────────────────────────
// Renaming in place for one folder view (the list or the grid): the slow click
// that starts it, the field laid over the entry's name, and the rename's whole
// life until the controller has answered.
//
// The field belongs to the view, not to a delegate. A listing refresh resets
// the model and rebuilds or reuses every delegate; a field inside one would be
// torn down with it and take the keyboard and what was typed along. Here it
// survives the rebuild, keeps its focus, and is placed again over whichever
// delegate now shows the entry, which it follows by key, never by row.
//
// A slow click is a press and click on the name of the entry that already was
// the only selection, with no button but the left and no modifier, followed by
// the platform's double-click interval with no second click: a double click
// still opens, a first click only selects, and a click on the icon or a drag
// never renames.
//
// The rename itself is asynchronous: `renamePath` answers by clearing
// `opError` and either setting it (refused: the editor stays open with what was
// typed, for the person to correct) or refreshing the folder (landed: the
// renamed entry becomes the selection). An empty or unchanged name ends the
// edit without asking.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    // The ListView or GridView this edits over; the field is laid over its
    // delegates' `nameLabel`.
    required property var view
    required property var controller
    required property var panel
    // The grid centres its names under the icon; the list starts them left.
    property bool centered: false

    // The entry being renamed, by key ("" when none), and what it was called.
    property string path: ""
    property string token: ""
    property string originalName: ""
    property bool directory: false
    // Return has handed the name to the controller and its answer is pending.
    property bool pending: false
    property string pendingName: ""
    readonly property bool editing: root.path.length > 0
    readonly property alias field: nameField

    // The slow click: the token whose press qualified, then the one waiting
    // out the double-click interval.
    property string armedToken: ""
    property string slowToken: ""

    // Beside the view, over it, rather than inside it. A ListView or GridView
    // is a focus scope that hands its focus to its current delegate whenever
    // the model resets; a field inside it lost the keyboard, and the edit, to
    // every refresh. Out here the view's own focus moves alone. It does not
    // scroll either, and nothing scrolls while it is open.
    parent: root.view.parent
    x: root.view.x
    y: root.view.y
    width: root.view.width
    height: root.view.height
    z: root.view.z + 1
    clip: true
    visible: root.editing

    // ── The slow click ───────────────────────────────────────────────────

    // Whether (x, y) in `from` lands on the painted text of `label`, not just
    // inside its box: a short name leaves most of its row to the icon's side
    // of the gesture.
    function hitsLabel(label, from, x, y) {
        if (!label || !label.visible)
            return false
        const point = label.mapFromItem(from, x, y)
        const width = Math.min(label.contentWidth, label.width)
        const left = label.horizontalAlignment === Text.AlignHCenter
                     ? (label.width - width) / 2 : 0
        const height = Math.max(label.contentHeight, label.height)
        return point.x >= left && point.x <= left + width
               && point.y >= 0 && point.y <= height
    }

    // Every press drops a click waiting out its interval: a second press is
    // the start of a double click, or of something else entirely.
    function press(token, label, from, x, y, button, modifiers) {
        slowClick.stop()
        root.slowToken = ""
        root.armedToken = (button === Qt.LeftButton && modifiers === Qt.NoModifier
                           && !root.editing
                           && root.panel.selectionCount === 1
                           && root.panel.isSelected(token)
                           && root.hitsLabel(label, from, x, y)) ? token : ""
    }

    function click(token) {
        const armed = root.armedToken === token
        root.armedToken = ""
        if (!armed)
            return
        root.slowToken = token
        slowClick.restart()
    }

    function doubleClick() {
        slowClick.stop()
        root.armedToken = ""
        root.slowToken = ""
    }

    Timer {
        id: slowClick
        interval: Application.styleHints.mouseDoubleClickInterval
        onTriggered: root.slowClickElapsed()
    }

    // The interval passed with no second click. The entry must still be the
    // only selection and the view still the keyboard's: a menu, a dialog or
    // an arrow key in between has moved on.
    function slowClickElapsed() {
        const token = root.slowToken
        root.slowToken = ""
        if (token.length === 0 || root.editing || !root.view.activeFocus
                || root.panel.selectionCount !== 1 || !root.panel.isSelected(token))
            return
        root.begin(root.controller.indexForToken(token))
    }

    // ── The edit ─────────────────────────────────────────────────────────

    // Starts renaming the entry at `index`, which becomes the selection.
    function begin(index) {
        if (index < 0 || !root.view.visible || root.controller.loading
                || root.controller.trashActive)
            return
        const key = root.controller.entryPath(index)
        if (key.length === 0)
            return
        root.doubleClick()
        root.token = root.controller.entryToken(index)
        root.originalName = root.controller.entryNames[index] ?? ""
        root.directory = root.controller.entryKind(index) === "directory"
                         || root.controller.entryTargetsDirectory(index)
        root.pending = false
        root.pendingName = ""
        root.path = key

        root.view.currentIndex = index
        root.panel.selectOnly(root.token)
        root.controller.selectToken(root.token)

        nameField.text = root.originalName
        root.reveal(index)
        root.place(index)
        nameField.forceActiveFocus()
        // The base name, as every desktop does: the extension stays as it
        // was unless the person reaches for it. A leading dot is part of the
        // name, and a folder's name has no extension at all.
        const dot = root.originalName.lastIndexOf(".")
        if (root.directory || dot <= 0)
            nameField.selectAll()
        else
            nameField.select(0, dot)
    }

    // Scrolls the entry at `index` into view and lays its delegate out now.
    function reveal(index) {
        root.view.positionViewAtIndex(index, ListView.Contain)
        root.view.forceLayout()
    }

    // Lays the field over the name of the delegate showing `index`, in that
    // name's own font, on its first line. False when no delegate shows it.
    function place(index) {
        const entry = root.view.itemAtIndex(index)
        if (!entry || !entry.nameLabel)
            return false
        const label = entry.nameLabel
        nameField.font.family = label.font.family
        nameField.font.pixelSize = label.font.pixelSize
        nameField.font.weight = label.font.weight
        const origin = label.mapToItem(root, 0, 0)
        const lineHeight = label.contentHeight / Math.max(1, label.lineCount)
        nameField.height = Math.round(lineHeight) + 2 * CelestinaTheme.spaceXs
        const wanted = label.width + nameField.leftPadding + nameField.rightPadding
        nameField.x = Math.max(CelestinaTheme.spaceXs,
                               origin.x - nameField.leftPadding)
        nameField.width = Math.min(wanted, root.width - nameField.x - CelestinaTheme.spaceXs)
        nameField.y = Math.round(origin.y + lineHeight / 2 - nameField.height / 2)
        return true
    }

    // Where the entry this edits is listed now, or -1. The token finds it at
    // once in a folder; a search hit's token is its row, so the key decides.
    function indexOfPath(key) {
        const byToken = root.controller.indexForToken(root.token)
        if (byToken >= 0 && root.controller.entryPath(byToken) === key)
            return byToken
        const count = root.controller.entryNames.length
        for (let index = 0; index < count; ++index) {
            if (root.controller.entryPath(index) === key)
                return index
        }
        return -1
    }

    // The renamed entry in the listing that landed: same parent key, the
    // name that was sent. -1 when it is not shown (a search's stale hits, a
    // name that starts hiding it).
    function indexOfRenamed() {
        const parentKey = root.path.substring(0, root.path.lastIndexOf("/"))
        const names = root.controller.entryNames
        for (let index = 0; index < names.length; ++index) {
            if (names[index] !== root.pendingName)
                continue
            const key = root.controller.entryPath(index)
            if (key.lastIndexOf("/") === parentKey.length && key.startsWith(parentKey + "/"))
                return index
        }
        return -1
    }

    function commit() {
        if (!root.editing || root.pending)
            return
        const name = nameField.text
        if (name.length === 0 || name === root.originalName) {
            root.end(true)
            return
        }
        // Pending before the call: a key the controller refuses is answered
        // synchronously, and that answer must find the edit waiting for it.
        root.pending = true
        root.pendingName = name
        root.controller.renamePath(root.path, name)
    }

    // The controller refused the name; its notice says why. The edit goes on
    // with what was typed.
    function refused() {
        root.pending = false
        root.pendingName = ""
        nameField.forceActiveFocus()
    }

    // A listing landed (or `loading` dropped). Runs after the whole
    // publication — the model is reset by the folder view's own handler of
    // the same burst — so the delegates it reads are the new ones.
    function settle() {
        if (!root.editing)
            return
        const index = root.indexOfPath(root.path)
        if (root.pending) {
            if (root.controller.loading)
                return
            // A folder that still lists the old name has not heard of the
            // rename yet; search hits and Recientes are never republished by
            // the refresh that follows one, so `loading` dropping is the answer.
            if (index >= 0 && !root.controller.searchActive && !root.controller.recentActive)
                return
            const renamed = root.indexOfRenamed()
            root.end(true)
            if (renamed >= 0) {
                const token = root.controller.entryToken(renamed)
                root.view.currentIndex = renamed
                root.panel.selectOnly(token)
                root.controller.selectToken(token)
                root.view.positionViewAtIndex(renamed, ListView.Contain)
            }
            return
        }
        if (index < 0) {
            root.end(false)
            return
        }
        root.reveal(index)
        root.place(index)
    }

    // The listing moved under the field (the scroll bar, a position restored
    // after a refresh): the field moves with its entry, and an entry that has
    // no delegate on screen any more is brought back, as nothing scrolls
    // away from an open rename.
    function follow() {
        if (root.editing && !root.place(root.indexOfPath(root.path)))
            root.settle()
    }

    // Ends the edit. `refocus` hands the keyboard back to the view (Return,
    // Escape); an edit ended by the person going elsewhere leaves it there.
    function end(refocus) {
        slowClick.stop()
        root.path = ""
        root.token = ""
        root.originalName = ""
        root.pending = false
        root.pendingName = ""
        nameField.text = ""
        if (refocus)
            root.view.forceActiveFocus()
        else
            Qt.callLater(root.keepKeyboard)
    }

    function cancel() {
        if (root.editing)
            root.end(false)
    }

    // An edit ended by a press somewhere that takes no focus would leave the
    // keyboard with nobody; the view takes it back then.
    function keepKeyboard() {
        const window = root.Window.window
        if (window && (!window.activeFocusItem || window.activeFocusItem === window.contentItem))
            root.view.forceActiveFocus()
    }

    // Leaving the field ends the edit, but switching to another window does
    // not: the field gets its focus back when the window does.
    function focusLeft() {
        if (root.editing && !nameField.activeFocus && root.Window.active)
            root.end(false)
    }

    Connections {
        target: root.controller
        enabled: root.editing
        function onOpErrorChanged() {
            if (root.pending && root.controller.opError.length > 0)
                root.refused()
        }
        function onLoadingChanged() {
            if (root.pending && !root.controller.loading)
                Qt.callLater(root.settle)
        }
        function onEntryNamesChanged() { Qt.callLater(root.settle) }
    }

    Connections {
        target: root.view
        enabled: root.editing
        function onVisibleChanged() { root.cancel() }
        function onWidthChanged() { Qt.callLater(root.settle) }
        function onHeightChanged() { Qt.callLater(root.settle) }
        function onContentYChanged() { Qt.callLater(root.follow) }
    }

    // A press that nothing took ends the edit too: the heading, the glass, a
    // label. Whatever does take a press either takes the focus (another entry,
    // the empty folder, a field, a menu), which ends the edit above, or
    // navigates, which takes the entry away. The catcher lies under the whole
    // window, so it hears only what fell through everything else: a handler
    // or an area over the window would claim the press and keep it from the
    // field itself, which then could not place its cursor or select.
    Item {
        parent: root.Window.window ? root.Window.window.contentItem : root
        anchors.fill: parent
        z: -1

        MouseArea {
            anchors.fill: parent
            enabled: root.editing
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
            onPressed: function(mouse) {
                mouse.accepted = false
                root.end(false)
            }
        }
    }

    CelestinaTextField {
        id: nameField

        leftPadding: CelestinaTheme.spaceXs
        rightPadding: CelestinaTheme.spaceXs
        topPadding: 0
        bottomPadding: 0
        verticalAlignment: TextInput.AlignVCenter
        horizontalAlignment: root.centered ? TextInput.AlignHCenter : TextInput.AlignLeft
        readOnly: root.pending
        Accessible.name: qsTr("Nuevo nombre")

        onActiveFocusChanged: {
            if (!activeFocus && root.editing)
                Qt.callLater(root.focusLeft)
        }

        // Return and Escape are taken here rather than through `accepted`:
        // an unaccepted key would travel on to the view and act on the entry.
        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                root.commit()
                event.accepted = true
            } else if (event.key === Qt.Key_Escape) {
                root.end(true)
                event.accepted = true
            }
        }

        // A right button inside the field would open a menu that takes the
        // focus, and with it the edit. Swallow it.
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.RightButton
            onPressed: nameField.forceActiveFocus()
        }
    }
}
