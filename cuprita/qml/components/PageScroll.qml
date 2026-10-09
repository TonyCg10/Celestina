import QtQuick
import org.celestina.cuprita 1.0

// A page's cards, stacked `spaceCardGap` apart, scrolling as one when they
// are taller than the window: the wheel through the suite's scroller and the
// shared scroll bar at the right edge. The lists inside the cards never
// scroll on their own; `reveal` brings a row the keyboard reached into view.
Item {
    id: page

    // The cards: they stack in the scrolled column.
    default property alias cards: column.data
    readonly property alias flickable: scroller

    // Scrolls the least that shows all of `item`.
    function reveal(item) {
        if (!item)
            return
        const top = item.mapToItem(column, 0, 0).y
        if (top < scroller.contentY)
            scroller.contentY = top
        else if (top + item.height > scroller.contentY + scroller.height)
            scroller.contentY = Math.min(top + item.height,
                                         scroller.contentHeight) - scroller.height
    }

    Flickable {
        id: scroller
        objectName: "pageScroller"
        anchors.fill: parent
        contentWidth: width
        contentHeight: column.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.VerticalFlick

        CelestinaWheelScroll { view: scroller }

        Column {
            id: column
            width: scroller.width
            spacing: CelestinaTheme.spaceCardGap
        }
    }

    CelestinaScrollBar {
        surface: scroller
        anchors.top: scroller.top
        anchors.bottom: scroller.bottom
        anchors.right: scroller.right
    }
}
