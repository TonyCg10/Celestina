import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The sidebar's bookmark and favourite rows: what each mouse button does, and
// what it does not. A bookmark is renamed by double click, so the first click
// can no longer navigate on its own — and while it is being edited the field
// owns keyboard and mouse, not the MouseArea that used to cover it. A
// favourite that no longer exists opens nothing, but its menu (the only place
// it can be removed from) still comes up on the right button.
TestCase {
    id: testCase
    name: "SidebarRows"
    width: 320
    height: 200
    visible: true
    when: windowShown

    property int editIndex: -1
    property int editRequests: 0
    property var openedTabs: []
    property int bookmarkMenus: 0
    property int favoriteMenus: 0

    QtObject {
        id: controllerStub

        property string markedKey: ""
        property int opens: 0
        property int reveals: 0
        property string lastOpened: ""

        function openKey(path) { opens++; lastOpened = path }
        function revealPath(path) { reveals++ }
    }

    QtObject {
        id: hostWindowStub

        property var activeController: controllerStub
        property real sidebarIconScale: 1.0
        property real sidebarTextScale: 1.0
        property int sidebarRowHeight: 34

        function openTab(path, foreground) {
            testCase.openedTabs.push({ path: path, foreground: foreground })
        }
    }

    SidebarBookmarkRow {
        id: bookmark
        x: 10
        y: 10
        width: 280
        hostWindow: hostWindowStub
        overlayParent: testCase
        rowIndex: 0
        bookmarkName: "CODIGO"
        bookmarkPath: "/home/toni/CODIGO"
        editing: testCase.editIndex === 0
        listDragIndex: -1
        listDropIndex: -1
        rowPitch: 36
        rowCount: 1
        onEditRequested: function(index) {
            testCase.editRequests++
            testCase.editIndex = index
        }
        onEditCancelled: testCase.editIndex = -1
        onRenameRequested: testCase.editIndex = -1
        onContextMenuRequested: testCase.bookmarkMenus++
    }

    SidebarFavoriteRow {
        id: missingFavorite
        x: 10
        y: 60
        width: 280
        hostWindow: hostWindowStub
        overlayParent: testCase
        entry: ({ name: "borrado", path: "/home/toni/borrado", kind: "missing" })
        onContextMenuRequested: testCase.favoriteMenus++
    }

    SidebarFavoriteRow {
        id: liveFavorite
        x: 10
        y: 110
        width: 280
        hostWindow: hostWindowStub
        overlayParent: testCase
        entry: ({ name: "notas", path: "/home/toni/notas", kind: "directory" })
        onContextMenuRequested: testCase.favoriteMenus++
    }

    // The system's double-click window, with a margin: the row's timer uses
    // exactly that interval.
    readonly property int doubleClickWindow:
            Application.styleHints.mouseDoubleClickInterval + 150

    function init() {
        editIndex = -1
        editRequests = 0
        openedTabs = []
        bookmarkMenus = 0
        favoriteMenus = 0
        controllerStub.opens = 0
        controllerStub.reveals = 0
        controllerStub.lastOpened = ""
        mouseMove(testCase, 300, 190)
    }

    // ── Bookmarks ──────────────────────────────────────────────────────────

    function test_a_single_click_opens_the_bookmark_after_the_double_click_window() {
        mouseClick(bookmark, 100, 17, Qt.LeftButton)
        // Not yet: the second click of a double click may be on its way.
        compare(controllerStub.opens, 0, "the first click navigated without waiting")
        tryCompare(controllerStub, "opens", 1, doubleClickWindow * 2)
        compare(controllerStub.lastOpened, "/home/toni/CODIGO")
    }

    function test_b_double_click_renames_without_navigating() {
        mouseDoubleClickSequence(bookmark, 100, 17, Qt.LeftButton)
        compare(editRequests, 1, "the double click did not ask to rename")
        compare(editIndex, 0)
        wait(doubleClickWindow)
        compare(controllerStub.opens, 0, "the double click navigated as well")
    }

    function test_c_middle_and_right_click_stay_immediate() {
        mouseClick(bookmark, 100, 17, Qt.MiddleButton)
        compare(openedTabs.length, 1, "the middle button did not open a tab")
        compare(openedTabs[0].foreground, false)

        mouseClick(bookmark, 100, 17, Qt.RightButton)
        compare(bookmarkMenus, 1, "the right button did not ask for the menu")

        wait(doubleClickWindow)
        compare(controllerStub.opens, 0, "middle or right also navigated")
    }

    // The reported case: clicking inside the rename field to place the caret
    // opened the bookmark and took the edit down with it.
    function test_d_clicks_inside_the_rename_field_do_not_navigate() {
        editIndex = 0
        tryVerify(function() { return bookmark.editing }, 1000)

        mouseClick(bookmark, 100, 17, Qt.LeftButton)
        mouseClick(bookmark, 140, 17, Qt.LeftButton)
        mouseClick(bookmark, 120, 17, Qt.RightButton)
        wait(doubleClickWindow)

        compare(controllerStub.opens, 0, "a click in the field opened the bookmark")
        compare(bookmarkMenus, 0, "a right click in the field opened the menu")
        compare(openedTabs.length, 0)
        // And the edit is still on: the click did not take the field's focus.
        compare(editIndex, 0, "the click inside the field cancelled the edit")
    }

    function test_e_return_on_the_focused_row_opens_it() {
        bookmark.forceActiveFocus()
        verify(bookmark.activeFocus)
        keyClick(Qt.Key_Return)
        compare(controllerStub.opens, 1, "Return did not open the bookmark")
        compare(controllerStub.lastOpened, "/home/toni/CODIGO")
    }

    // ── Favourites ─────────────────────────────────────────────────────────

    function test_f_a_missing_favorite_still_offers_its_menu() {
        mouseClick(missingFavorite, 100, 17, Qt.RightButton)
        compare(favoriteMenus, 1, "the missing favourite lost its menu")

        mouseClick(missingFavorite, 100, 17, Qt.LeftButton)
        mouseClick(missingFavorite, 100, 17, Qt.MiddleButton)
        compare(controllerStub.opens, 0, "a missing favourite opened something")
        compare(controllerStub.reveals, 0)
        compare(openedTabs.length, 0)
        // Nor does the keyboard reach it: there is nothing to activate.
        compare(missingFavorite.activeFocusOnTab, false)
    }

    function test_g_a_live_favorite_opens_with_every_gesture() {
        mouseClick(liveFavorite, 100, 17, Qt.LeftButton)
        compare(controllerStub.opens, 1)
        compare(controllerStub.lastOpened, "/home/toni/notas")

        mouseClick(liveFavorite, 100, 17, Qt.MiddleButton)
        compare(openedTabs.length, 1)

        liveFavorite.forceActiveFocus()
        keyClick(Qt.Key_Return)
        compare(controllerStub.opens, 2, "Return did not open the favourite")
    }
}
