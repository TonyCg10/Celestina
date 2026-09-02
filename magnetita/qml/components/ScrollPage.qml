import QtQuick
import QtQuick.Window
import org.celestina.magnetita 1.0

// A page that scrolls. Both of Magnetita's pages are a column of cards that
// can outgrow the window, and both owe the same three things: a viewport that
// stops at its bounds, the focused control kept in view when Tab moves it
// under the fold, and PageUp/PageDown/Home/End when the page itself has the
// keyboard. One owner for that rule; the pages only supply the column.
Flickable {
    id: root

    // The page's rows. They land in the column, so `parent.width` inside a
    // row is the column's, which is the page's.
    default property alias content: column.data
    property alias spacing: column.spacing

    contentWidth: width
    contentHeight: column.implicitHeight
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    // Reachable by Tab on its own, so the paging keys work with no focused
    // child — a page of plain text has none.
    activeFocusOnTab: true

    function ensureFocusVisible(item) {
        if (!item)
            return
        let ancestor = item
        while (ancestor && ancestor !== column)
            ancestor = ancestor.parent
        if (ancestor !== column)
            return

        const point = item.mapToItem(column, 0, 0)
        const top = point.y
        const bottom = top + item.height
        if (top < contentY)
            contentY = Math.max(0, top - CelestinaTheme.spaceSm)
        else if (bottom > contentY + height)
            contentY = Math.min(Math.max(0, contentHeight - height),
                                bottom - height + CelestinaTheme.spaceSm)
    }

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_PageDown) {
            contentY = Math.min(Math.max(0, contentHeight - height),
                                contentY + height * 0.8)
        } else if (event.key === Qt.Key_PageUp) {
            contentY = Math.max(0, contentY - height * 0.8)
        } else if (event.key === Qt.Key_Home) {
            contentY = 0
        } else if (event.key === Qt.Key_End) {
            contentY = Math.max(0, contentHeight - height)
        } else {
            return
        }
        event.accepted = true
    }

    Connections {
        target: root.Window.window

        function onActiveFocusItemChanged() {
            const item = root.Window.window
                         ? root.Window.window.activeFocusItem : null
            Qt.callLater(function() { root.ensureFocusVisible(item) })
        }
    }

    Column {
        id: column
        width: root.width
    }
}
