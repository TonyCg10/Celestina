import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// Renaming in place in the folder list and grid (SID-H1-S).
//
// A slow second click on the name of the entry that is already the only
// selection opens an editor over that name once the double-click interval has
// passed with no second click; a click on an unselected entry, on the icon, or
// the first half of a double click never does. F2 and the entry menu's
// «Renombrar» open the same editor. The editor selects the base name, takes a
// pointer drag as a text selection (nothing under it may take the drag), hands
// Return to the controller and stays open with what was typed when the
// controller refuses it, and Escape, a click elsewhere or a vanished entry end
// it without a call.
//
// The controller is a stub: a rename "lands" when a test publishes the listing
// the controller would publish after it (loading drops, the names and the
// model change), and it is refused when a test sets `opError`.
TestCase {
    id: testCase
    name: "InlineRename"
    width: 720
    height: 520
    visible: true
    when: windowShown

    readonly property int slowClickWait: Qt.styleHints.mouseDoubleClickInterval + 150

    ListModel {
        id: entries
    }

    function rowsFixture() {
        return [
            { name: "Documents", token: "1", kind: "directory", path: "/k/Documents" },
            { name: "report.final.pdf", token: "2", kind: "file", path: "/k/report.final.pdf" },
            { name: "notes.txt", token: "3", kind: "file", path: "/k/notes.txt" },
            { name: "Photos.2024", token: "4", kind: "directory", path: "/k/Photos.2024" },
            { name: ".profile", token: "5", kind: "file", path: "/k/.profile" },
            { name: "zeta.md", token: "6", kind: "file", path: "/k/zeta.md" }
        ]
    }

    function publish(rows) {
        entries.clear()
        const names = []
        for (let index = 0; index < rows.length; ++index) {
            const row = rows[index]
            entries.append({
                name: row.name, token: row.token, kind: row.kind,
                subtitle: "", path: row.path,
                isDirectory: row.kind === "directory",
                sizeText: "", dateText: "", section: ""
            })
            names.push(row.name)
        }
        controllerStub.rows = rows
        controllerStub.entryNames = names
    }

    // What the controller publishes once a rename it accepted has landed: the
    // refresh drops `loading`, then the listing with the new name and token.
    function landRename(fromPath, toName, toToken) {
        const rows = controllerStub.rows.map(function(row) {
            return row.path === fromPath
                    ? { name: toName, token: toToken, kind: row.kind, path: "/k/" + toName }
                    : row
        })
        controllerStub.loading = true
        controllerStub.loading = false
        publish(rows)
    }

    QtObject {
        id: controllerStub

        property var rows: []
        property var entryNames: []
        property var cutPaths: []
        property string selectedToken: ""
        property string opError: ""
        property bool loading: false
        property bool searchActive: false
        property bool trashActive: false
        property bool recentActive: false
        property bool canGoUp: false
        property bool canGoForward: false
        property bool canUndo: false
        property bool opRunning: false
        property int phoneRevision: 0
        property var phoneNames: []
        property var renames: []

        function entryToken(index) { return rows[index] ? rows[index].token : "" }
        function entryPath(index) { return rows[index] ? rows[index].path : "" }
        function entryKind(index) { return rows[index] ? rows[index].kind : "" }
        function entryTargetsDirectory(index) { return rows[index] ? rows[index].kind === "directory" : false }
        function indexForToken(token) {
            return rows.findIndex(function(row) { return row.token === token })
        }
        function selectToken(token) { selectedToken = token }
        function renamePath(path, name) {
            opError = ""
            renames.push({ path: path, name: name })
        }
        function closeSearch() { }
        function goUp() { }
        function areArchives(keys) { return false }
        function phoneInfo(index) { return ["", "", "", "0"] }
        function trashPath(path) { }
        function trashPaths(paths) { }
    }

    QtObject {
        id: iconsStub
        function mediaKind(name) { return "" }
        function mediaIconName(kind, media, path) { return kind === "directory" ? "folder" : "text-x-generic" }
        function entryOwnIcon(path) { return "" }
        function entryIconTone(kind) { return CelestinaIcon.File }
        function iconTint(path) { return CelestinaTheme.clear }
        function isFavorite(path) { return false }
        function customIconAccent(path) { return "" }
    }

    QtObject {
        id: panelStub

        property string viewMode: "list"
        property int listRowHeight: 48
        property int gridCellWidth: 120
        property int gridCellHeight: 140
        property var icons: iconsStub
        property var selectedTokens: ({})
        property int selectionCount: 0
        property string anchorToken: ""

        function isSelected(token) { return token.length > 0 && selectedTokens[token] === true }
        function selectOnly(token) {
            const s = {}
            s[token] = true
            selectedTokens = s
            selectionCount = 1
            anchorToken = token
        }
        function clearSelection() {
            selectedTokens = ({})
            selectionCount = 0
        }
        function toggleSelection(token) {
            const s = Object.assign({}, selectedTokens)
            if (s[token])
                delete s[token]
            else
                s[token] = true
            selectedTokens = s
            selectionCount = Object.keys(s).length
        }
        function selectRange(index) { }
        function isEntryDrag(drag) { return false }
        function dropOnto(path, drop) { }
        function startEntryDrag(path, label, isDirectory, glyph, handler) { testCase.dragsStarted++ }
        function actingCount(token) { return 1 }
        function operativePaths(token, path) { return [path] }
        function selectedPaths() { return [] }
        function copySelection(token, path, cut) { }
        function trashSelection(token, path) { }
    }

    property int dragsStarted: 0

    QtObject {
        id: hostStub
        property real contentIconScale: 1.0
        property real contentTextScale: 1.0
        property var activated: []
        function activateEntry(controller, token) { activated.push(token) }
    }

    QtObject {
        id: headingStub
        function advance(delta, notch) { }
    }

    Item {
        id: ghost
        width: 10
        height: 10
        property string path: ""
        Drag.active: false
        Drag.dragType: Drag.Automatic
    }

    FolderListView {
        id: list
        x: 0
        y: 0
        width: 460
        height: 220
        controller: controllerStub
        entryModel: entries
        panel: panelStub
        hostWindow: hostStub
        ghost: ghost
        overlayParent: testCase
        contentTopMargin: 0
        contentBottomInset: 0
        heading: headingStub
    }

    // The grid is hidden by its own rule unless the panel is in grid mode;
    // the grid tests switch the mode.
    FolderGridView {
        id: grid
        x: 0
        y: 0
        width: 460
        height: 220
        controller: controllerStub
        entryModel: entries
        panel: panelStub
        hostWindow: hostStub
        ghost: ghost
        overlayParent: testCase
        contentTopMargin: 0
        contentBottomInset: 0
        heading: headingStub
    }

    QtObject {
        id: topBarStub
        property var activeView: panelStub.viewMode === "grid" ? grid : list
    }

    FolderShortcuts {
        anchors.fill: parent
        viewActive: !list.renaming && !grid.renaming
        canGoBackOrLeave: false
        controller: controllerStub
        panel: panelStub
        topBar: topBarStub
    }

    EntryContextMenu {
        id: menu
        backdropSource: testCase
        controller: controllerStub
        panel: panelStub
        renameView: topBarStub.activeView
    }

    // Something outside the views that takes no focus when pressed.
    Rectangle {
        id: elsewhere
        x: 480
        y: 20
        width: 200
        height: 200
        color: "gray"
    }

    function activeView() { return panelStub.viewMode === "grid" ? grid : list }
    function delegateAt(index) {
        const view = activeView()
        view.positionViewAtIndex(index, ListView.Contain)
        view.forceLayout()
        return view.itemAtIndex(index)
    }
    function nameOf(index) { return delegateAt(index).nameLabel }
    function editorField() { return activeView().renamer.field }

    function clickName(index) {
        const label = nameOf(index)
        mouseClick(label, Math.min(12, label.width / 2), label.height / 2)
    }
    function select(index) {
        clickName(index)
        compare(panelStub.selectionCount, 1)
        verify(panelStub.isSelected(controllerStub.entryToken(index)))
        // A second click inside the interval would be a double click.
        wait(slowClickWait)
    }
    function startEditing(index) {
        select(index)
        clickName(index)
        tryVerify(function() { return activeView().renaming }, slowClickWait + 1000)
        tryVerify(function() { return editorField().activeFocus })
    }

    function init() {
        menu.close()
        panelStub.viewMode = "list"
        panelStub.clearSelection()
        controllerStub.renames = []
        controllerStub.opError = ""
        controllerStub.loading = false
        controllerStub.searchActive = false
        controllerStub.trashActive = false
        hostStub.activated = []
        dragsStarted = 0
        list.renamer.cancel()
        grid.renamer.cancel()
        publish(rowsFixture())
        list.currentIndex = -1
        grid.currentIndex = -1
        list.positionViewAtBeginning()
        wait(slowClickWait)
        mouseMove(elsewhere, 5, 5)
    }

    // ── The slow click ─────────────────────────────────────────────────────

    function test_a_slow_click_on_the_selected_name_edits_after_the_interval() {
        select(2)
        clickName(2)
        // Not before the interval: a second click may still come.
        verify(!list.renaming, "the editor opened before the double-click interval")
        tryVerify(function() { return list.renaming }, slowClickWait + 1000)
        compare(list.renamer.path, "/k/notes.txt")
        compare(editorField().text, "notes.txt")
        verify(editorField().activeFocus)
    }

    function test_b_a_click_on_an_unselected_entry_only_selects_it() {
        clickName(2)
        verify(panelStub.isSelected("3"))
        wait(slowClickWait + 200)
        verify(!list.renaming, "a first click on an entry renamed it")
    }

    function test_c_a_double_click_on_the_selected_entry_opens_it_and_never_edits() {
        select(2)
        const label = nameOf(2)
        mouseDoubleClickSequence(label, 10, label.height / 2)
        compare(hostStub.activated, ["3"], "the double click did not open the entry")
        wait(slowClickWait + 200)
        verify(!list.renaming, "the double click also started a rename")
    }

    function test_d_a_click_on_the_icon_never_edits() {
        select(2)
        const row = delegateAt(2)
        mouseClick(row, 24, row.height / 2)
        wait(slowClickWait + 200)
        verify(!list.renaming, "a click on the icon renamed the entry")
    }

    function test_e_a_click_with_a_modifier_never_edits() {
        select(2)
        const label = nameOf(2)
        mouseClick(label, 10, label.height / 2, Qt.LeftButton, Qt.ControlModifier)
        wait(slowClickWait + 200)
        verify(!list.renaming, "a Ctrl click renamed the entry")
    }

    function test_f_a_slow_click_is_dropped_when_the_selection_moves_on() {
        select(2)
        clickName(2)
        keyClick(Qt.Key_Down)
        wait(slowClickWait + 200)
        verify(!list.renaming, "the rename opened on an entry no longer selected")
    }

    // ── F2 and the menu ────────────────────────────────────────────────────

    function test_g_f2_edits_the_current_entry() {
        select(1)
        keyClick(Qt.Key_F2)
        tryVerify(function() { return list.renaming })
        compare(list.renamer.path, "/k/report.final.pdf")
        tryVerify(function() { return editorField().activeFocus })
    }

    function test_h_the_menu_entry_edits_in_place() {
        select(3)
        menu.targetToken = "4"
        menu.targetName = "Photos.2024"
        menu.targetDirectory = true
        menu.targetPath = "/k/Photos.2024"
        menu.popup(testCase, Qt.point(500, 260))
        tryCompare(menu, "opened", true)
        let entry = null
        for (let index = 0; index < menu.count; ++index) {
            if (menu.itemAt(index) && menu.itemAt(index).objectName === "renameEntry")
                entry = menu.itemAt(index)
        }
        verify(entry !== null && entry.visible, "no visible rename entry in the menu")
        mouseClick(entry)
        tryCompare(menu, "visible", false)
        // The menu's own close handler still runs beside the rename's.
        compare(menu.pointerShield, null, "the closed menu left its pointer shield up")
        tryVerify(function() { return list.renaming })
        compare(list.renamer.path, "/k/Photos.2024")
        // The menu hands the focus back as it closes; the editor keeps it.
        tryVerify(function() { return editorField().activeFocus })
        wait(200)
        verify(list.renaming, "the closing menu took the focus and ended the rename")
    }

    // ── The editor ─────────────────────────────────────────────────────────

    function test_i_the_extension_is_left_out_of_the_first_selection() {
        list.beginRename(1)
        tryVerify(function() { return editorField().activeFocus })
        compare(editorField().selectedText, "report.final")
        list.renamer.cancel()
        list.beginRename(4)
        tryVerify(function() { return editorField().activeFocus })
        compare(editorField().selectedText, ".profile", "a dotfile is all base name")
        list.renamer.cancel()
        list.beginRename(3)
        tryVerify(function() { return editorField().activeFocus })
        compare(editorField().selectedText, "Photos.2024", "a folder's name is selected whole")
    }

    function test_j_return_renames_and_selects_the_renamed_entry() {
        startEditing(2)
        editorField().selectAll()
        keyClick(Qt.Key_A)
        keyClick(Qt.Key_B)
        keyClick(Qt.Key_Return)
        compare(controllerStub.renames.length, 1)
        compare(controllerStub.renames[0].path, "/k/notes.txt")
        compare(controllerStub.renames[0].name, "ab")
        landRename("/k/notes.txt", "ab", "9")
        tryVerify(function() { return !list.renaming })
        verify(panelStub.isSelected("9"), "the renamed entry is not the selection")
        compare(panelStub.selectionCount, 1)
        compare(controllerStub.selectedToken, "9")
        compare(list.currentIndex, 2)
        tryVerify(function() { return list.activeFocus }, 1000, "the list did not get the keyboard back")
    }

    function test_k_an_empty_or_unchanged_name_ends_without_a_call() {
        startEditing(2)
        keyClick(Qt.Key_Return)
        verify(!list.renaming)
        compare(controllerStub.renames.length, 0, "an unchanged name was sent")
        startEditing(2)
        editorField().selectAll()
        keyClick(Qt.Key_Delete)
        keyClick(Qt.Key_Return)
        verify(!list.renaming)
        compare(controllerStub.renames.length, 0, "an empty name was sent")
    }

    function test_l_a_refused_name_keeps_the_editor_open_with_the_text() {
        startEditing(2)
        editorField().selectAll()
        keyClick(Qt.Key_Z)
        keyClick(Qt.Key_E)
        keyClick(Qt.Key_T)
        keyClick(Qt.Key_A)
        keyClick(Qt.Key_Period)
        keyClick(Qt.Key_M)
        keyClick(Qt.Key_D)
        keyClick(Qt.Key_Return)
        compare(controllerStub.renames.length, 1)
        // The controller answers the clash with its usual notice.
        controllerStub.opError = "an entry with that name already exists"
        tryVerify(function() { return editorField().activeFocus })
        verify(list.renaming, "the refused rename closed the editor")
        compare(editorField().text, "zeta.md")
        verify(!editorField().readOnly)
        verify(panelStub.isSelected("3"), "the selection left the entry")
        // And it can be corrected and sent again.
        keyClick(Qt.Key_Backspace)
        keyClick(Qt.Key_Backspace)
        keyClick(Qt.Key_T)
        keyClick(Qt.Key_X)
        keyClick(Qt.Key_T)
        keyClick(Qt.Key_Return)
        compare(controllerStub.renames.length, 2)
        compare(controllerStub.renames[1].name, "zeta.txt")
    }

    function test_m_escape_cancels_without_a_call() {
        startEditing(2)
        keyClick(Qt.Key_X)
        keyClick(Qt.Key_Escape)
        verify(!list.renaming)
        compare(controllerStub.renames.length, 0)
        verify(panelStub.isSelected("3"))
        tryVerify(function() { return list.activeFocus })
    }

    function test_n_a_click_elsewhere_cancels() {
        startEditing(2)
        keyClick(Qt.Key_X)
        mouseClick(elsewhere, 50, 50)
        tryVerify(function() { return !list.renaming })
        compare(controllerStub.renames.length, 0)
    }

    function test_o_a_click_on_another_entry_cancels() {
        startEditing(2)
        clickName(0)
        tryVerify(function() { return !list.renaming })
        compare(controllerStub.renames.length, 0)
        verify(panelStub.isSelected("1"))
    }

    // Item 1 of the unit: a drag across the name selects text. The steps are a
    // real mouse's — a few pixels each — because a drag that jumps past the
    // start-drag distance on its first move hides a handler that steals the
    // grab before the field claims it.
    function dragAcross(field) {
        const y = field.height / 2
        mousePress(field, field.leftPadding + 1, y, Qt.LeftButton)
        for (let x = field.leftPadding + 3; x < field.width - field.rightPadding; x += 2)
            mouseMove(field, x, y)
        mouseRelease(field, field.width - field.rightPadding - 1, y, Qt.LeftButton)
    }

    function test_p_a_drag_across_the_name_selects_text() {
        startEditing(1)
        editorField().deselect()
        dragAcross(editorField())
        verify(list.renaming, "the drag ended the rename")
        compare(editorField().selectedText, "report.final.pdf")
        compare(dragsStarted, 0, "the drag picked the row up")
    }

    function test_q_scrolling_and_keys_stay_with_the_editor() {
        startEditing(2)
        verify(!list.interactive, "the list still flicks under the editor")
        const before = list.contentY
        mouseWheel(editorField(), 10, 10, 0, -360)
        wait(100)
        compare(list.contentY, before, "the wheel scrolled the list under the editor")
        keyClick(Qt.Key_Down)
        compare(list.currentIndex, 2, "the arrow moved the selection")
        keyClick(Qt.Key_Q)
        verify(panelStub.isSelected("3"), "type-to-select moved the selection")
        compare(editorField().text.indexOf("q") >= 0, true)
    }

    function test_r_the_editor_follows_its_entry_through_a_refresh() {
        startEditing(5)
        keyClick(Qt.Key_End)
        keyClick(Qt.Key_X)
        const rows = rowsFixture()
        rows.splice(0, 0, { name: "Aardvark", token: "7", kind: "file", path: "/k/Aardvark" })
        publish(rows)
        wait(50)
        verify(list.renaming, "a refresh ended the rename")
        compare(list.renamer.path, "/k/zeta.md")
        compare(editorField().text, "zeta.mdx")
        const row = delegateAt(6)
        const top = row.mapToItem(list, 0, 0).y
        const fieldTop = editorField().mapToItem(list, 0, 0).y
        verify(fieldTop >= top && fieldTop < top + row.height,
               "the editor stayed over the old row: " + fieldTop + " vs " + top)
        verify(editorField().activeFocus)
    }

    function test_s_the_editor_ends_when_its_entry_disappears() {
        startEditing(5)
        const rows = rowsFixture()
        rows.splice(5, 1)
        publish(rows)
        tryVerify(function() { return !list.renaming })
        compare(controllerStub.renames.length, 0)
    }

    function test_t_the_field_is_named_for_assistive_technology() {
        startEditing(2)
        compare(editorField().Accessible.name, qsTr("Nuevo nombre"))
    }

    // ── The grid ───────────────────────────────────────────────────────────

    function test_u_the_grid_edits_on_a_slow_click_and_selects_by_drag() {
        panelStub.viewMode = "grid"
        wait(50)
        startEditing(1)
        compare(grid.renamer.path, "/k/report.final.pdf")
        compare(editorField().selectedText, "report.final")
        editorField().deselect()
        dragAcross(editorField())
        compare(editorField().selectedText, "report.final.pdf")
        compare(dragsStarted, 0, "the drag picked the cell up")
        keyClick(Qt.Key_Escape)
        verify(!grid.renaming)
    }

    function test_v_switching_the_view_ends_the_rename() {
        startEditing(2)
        panelStub.viewMode = "grid"
        tryVerify(function() { return !list.renaming })
        compare(controllerStub.renames.length, 0)
    }
}
