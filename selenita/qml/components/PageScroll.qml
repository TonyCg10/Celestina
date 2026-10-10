import QtQuick
import org.celestina.selenita 1.0

// The window's cards in one scrolled column: the wheel through the suite's
// scroller, the shared bar on the right. `reveal` scrolls the least that
// shows an item whole.
Item {
    id: page

    default property alias cards: column.data
    readonly property alias flickable: scroller

    function reveal(item: Item) {
        if (!item)
            return
        const top = item.mapToItem(column, 0, 0).y
        if (top < scroller.contentY)
            scroller.contentY = top
        else if (top + item.height > scroller.contentY + scroller.height)
            scroller.contentY = Math.max(0, Math.min(top + item.height, scroller.contentHeight)
                                         - scroller.height)
    }

    Flickable {
        id: scroller
        objectName: "pageScroller"
        anchors.fill: parent
        contentWidth: width
        contentHeight: column.implicitHeight + CelestinaTheme.windowMargin * 2
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.VerticalFlick

        CelestinaWheelScroll { view: scroller }

        Column {
            id: column
            x: CelestinaTheme.windowMargin
            y: CelestinaTheme.windowMargin
            width: scroller.width - CelestinaTheme.windowMargin * 2
            spacing: CelestinaTheme.spaceXl
        }
    }

    CelestinaScrollBar {
        surface: scroller
        anchors.top: scroller.top
        anchors.bottom: scroller.bottom
        anchors.right: scroller.right
    }
}
