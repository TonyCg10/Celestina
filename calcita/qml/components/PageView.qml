pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Pdf
import QtQuick.Shapes
import org.celestina.calcita 1.0

// The continuous pages of one document, in the suite's grammar: each page is
// QtPdf's image of it, with the search hits, the selection and the links laid
// over it in theme tokens. QtPdf renders, finds and selects; this view only
// places. Internal links move the view; an external link is only reported
// (`externalLinkRequested`) so the window can ask before anything leaves.
//
// QtPdf's own `PdfMultiPageView` opens an external link at once and paints
// hits and selection in its own colours, not the theme's, so Calcita lays
// its pages out itself. Each page's top is computed once per document and
// scale (`tops`), so placing a page, finding the page at a position (a
// binary search) and the content height cost nothing per scroll. Every page
// has an empty shell item; its image, hits, selection and links are built
// only while it lies in the near range (`firstNear`..`lastNear`, a viewport
// above and two below), which is the only thing a scroll updates.
//
// The pointer selects text: the view does not flick on a drag, so a drag on
// a page is always a selection; it scrolls with the wheel, the scroll bar
// and the keyboard.
Flickable {
    id: pageArea

    // Owned by the window's owner and never destroyed while Calcita runs
    // (QtPdf's image reader thread may still be reading one of its pages).
    required property PdfDocument document
    // Pixels per point.
    property real renderScale: 1
    // The 0-based page a jump went to, or the page under the reading line
    // after any other scroll; -1 without pages.
    property int currentPage: -1
    // The text selected with the pointer, on one page at a time.
    property string selectedText: ""
    property int selectionPage: -1
    // The search: the window sets `searchModel.searchString`.
    readonly property alias searchModel: hits

    // The top of each page in pixels, at `laidOutScale`.
    property var tops: []
    property real laidOutScale: 1
    readonly property int pageGap: 6
    // The paper every page is drawn on. QtPdf renders a page with a clear
    // background, so without it the ink would sit on the window's glass:
    // the canvas is glass, but a page is content and is opaque paper. The
    // suite has no paper token of its own; `iconSheet` is its one sheet of
    // paper (the near-white, warm leaf of the folder icon), so the pages
    // take that rather than a second white. The reading mode's layer
    // inverts the paper with the ink, so it reads dark there.
    readonly property color paper: CelestinaTheme.iconSheet
    // The pages whose content is built.
    property int firstNear: -1
    property int lastNear: -1
    // How many pages have their content built now.
    readonly property int builtPages: pageArea.lastNear >= pageArea.firstNear && pageArea.firstNear >= 0
                                      ? pageArea.lastNear - pageArea.firstNear + 1 : 0

    // A jump is moving the view, so the page it went to stays current even
    // where the view cannot bring that page to the top.
    property bool jumping: false
    // A jump asked for before the pages were laid out.
    property int pendingPage: -1
    property point pendingLocation: Qt.point(0, 0)
    // The hit count last seen, so only a new search's first hits move the
    // view while QtPdf fills the results page by page.
    property int seenHits: 0

    // The space kept above a jumped-to location or hit.
    readonly property int jumpMargin: CelestinaTheme.spaceXl
    readonly property size firstPageSize: pageArea.document.status === PdfDocument.Ready
                                          && pageArea.document.pageCount > 0
                                          ? pageArea.document.pagePointSize(0)
                                          : Qt.size(1, 1)
    readonly property int count: pages.count

    // An external link was tapped; nothing has been opened.
    signal externalLinkRequested(string url)

    function scaleToWidth(width, height) {
        pageArea.renderScale = width / pageArea.firstPageSize.width
    }

    function scaleToPage(width, height) {
        pageArea.renderScale = Math.min(width / pageArea.firstPageSize.width,
                                        height / pageArea.firstPageSize.height)
    }

    function goToPage(page) {
        pageArea.goToLocation(page, Qt.point(0, 0))
    }

    function maxContentY() {
        return Math.max(0, pageArea.contentHeight - pageArea.height)
    }

    function moveTo(y) {
        pageArea.jumping = true
        pageArea.contentY = Math.max(0, Math.min(pageArea.maxContentY(), y))
        pageArea.jumping = false
    }

    // Shows `location` (in points) of the 0-based `page` near the top.
    function goToLocation(page, location) {
        if (page < 0)
            return
        if (page >= pageArea.tops.length) {
            pageArea.pendingPage = page
            pageArea.pendingLocation = location
            return
        }
        const offset = location.y > 0 ? location.y * pageArea.renderScale - pageArea.jumpMargin : 0
        pageArea.moveTo(pageArea.tops[page] + offset)
        pageArea.currentPage = page
    }

    // The index of the item under content position `y` (the one above in a
    // gap), or -1 without pages: a binary search over `tops`.
    function pageAt(y) {
        const tops = pageArea.tops
        if (tops.length === 0)
            return -1
        let low = 0
        let high = tops.length - 1
        while (low < high) {
            const middle = (low + high + 1) >> 1
            if (tops[middle] <= y)
                low = middle
            else
                high = middle - 1
        }
        return low
    }

    function updateNear() {
        if (pageArea.tops.length === 0) {
            pageArea.firstNear = -1
            pageArea.lastNear = -1
            return
        }
        pageArea.firstNear = pageArea.pageAt(pageArea.contentY - pageArea.height)
        pageArea.lastNear = pageArea.pageAt(pageArea.contentY + pageArea.height * 2)
    }

    // Computes every page's top at the current scale, keeping the point at
    // the top of the view where it was in the page it belongs to.
    function relayout() {
        const ready = pageArea.document.status === PdfDocument.Ready
        const count = ready ? pageArea.document.pageCount : 0
        const oldTops = pageArea.tops
        const oldScale = pageArea.laidOutScale
        const anchor = pageArea.pageAt(pageArea.contentY)
        const within = anchor >= 0 ? (pageArea.contentY - oldTops[anchor]) / oldScale : 0
        const tops = []
        let y = 0
        for (let page = 0; page < count; page += 1) {
            tops.push(y)
            y += pageArea.document.pagePointSize(page).height * pageArea.renderScale + pageArea.pageGap
        }
        pageArea.laidOutScale = pageArea.renderScale
        pageArea.tops = tops
        pageArea.contentHeight = Math.max(0, y - pageArea.pageGap)
        if (anchor >= 0 && anchor < count)
            pageArea.moveTo(tops[anchor] + within * pageArea.renderScale)
        pageArea.updateNear()
        pageArea.pagesChanged()
    }

    // A scroll the person made: the page under the reading line a
    // third down, or the last page at the end, becomes the current one.
    function followScroll() {
        if (pageArea.count < 1)
            return
        const end = pageArea.contentY >= pageArea.contentHeight - pageArea.height - 1
        pageArea.currentPage = end ? pageArea.count - 1
                                   : pageArea.pageAt(pageArea.contentY + pageArea.height / 3)
    }

    function showCurrentHit() {
        if (hits.count < 1 || hits.currentResult < 0 || hits.currentPage < 0)
            return
        const rect = hits.currentResultBoundingRect
        pageArea.goToLocation(hits.currentPage, Qt.point(rect.x, rect.y))
    }

    // Enter, F3 and the card's arrows: makes `hit` current and shows it.
    function showHit(hit) {
        if (hit < 0 || hit >= hits.count)
            return
        hits.currentResult = hit
        pageArea.showCurrentHit()
    }

    function clearSelection() {
        pageArea.selectionPage = -1
        pageArea.selectedText = ""
    }

    function pagesChanged() {
        if (pageArea.tops.length === 0) {
            pageArea.currentPage = -1
            pageArea.clearSelection()
        } else if (pageArea.pendingPage >= 0 && pageArea.pendingPage < pageArea.tops.length) {
            const page = pageArea.pendingPage
            pageArea.pendingPage = -1
            pageArea.goToLocation(page, pageArea.pendingLocation)
        } else if (pageArea.currentPage < 0) {
            pageArea.currentPage = 0
        }
    }

    objectName: "pageView"
    clip: true
    interactive: false
    boundsBehavior: Flickable.StopAtBounds
    contentWidth: Math.max(pageArea.width, pageArea.document.maxPageWidth * pageArea.renderScale)
    activeFocusOnTab: true
    Accessible.role: Accessible.Document
    Accessible.name: qsTr("Páginas")

    onRenderScaleChanged: pageArea.relayout()
    onHeightChanged: pageArea.updateNear()
    onContentYChanged: {
        pageArea.updateNear()
        if (!pageArea.jumping)
            pageArea.followScroll()
    }

    Connections {
        target: pageArea.document
        function onStatusChanged() { pageArea.relayout() }
        function onPageCountChanged() { pageArea.relayout() }
    }

    Component.onCompleted: pageArea.relayout()

    PdfSearchModel {
        id: hits
        document: pageArea.document
    }

    // A new search shows its first hit, which QtPdf makes current without a
    // change signal; later hits arriving as QtPdf reads on leave the view
    // alone. `count` is newer than the type's export, so its signal is
    // reached through a Connections.
    Connections {
        target: hits
        function onCountChanged() {
            const first = pageArea.seenHits === 0 && hits.count > 0
            pageArea.seenHits = hits.count
            if (first)
                pageArea.showCurrentHit()
        }
    }

    CelestinaWheelScroll {
        view: pageArea
    }

    Repeater {
        id: pages
        model: pageArea.document.status === PdfDocument.Ready ? pageArea.document.pageCount : 0

        delegate: Item {
            id: pageItem

            required property int index
            readonly property size pointSize: pageArea.document.pagePointSize(pageItem.index)

            y: pageItem.index < pageArea.tops.length ? pageArea.tops[pageItem.index] : 0
            width: pageArea.contentWidth
            height: pageItem.pointSize.height * pageArea.renderScale

            Loader {
                objectName: "page" + pageItem.index
                x: Math.max(0, (pageItem.width - width) / 2)
                width: pageItem.pointSize.width * pageArea.renderScale
                height: pageItem.height
                active: pageItem.index >= pageArea.firstNear && pageItem.index <= pageArea.lastNear
                sourceComponent: paperComponent
            }

            Component {
                id: paperComponent

                Item {
                    id: paper

                    // How many hits are marked on this page.
                    readonly property int markedHits: pageHits.paths.length

                    // QtPdf reports a page's hits through a call, not a
                    // property.
                    function refreshHits() {
                        pageHits.paths = hits.searchString.length > 0
                                         ? hits.boundingPolygonsOnPage(pageItem.index) : []
                    }

                    Component.onCompleted: paper.refreshHits()

                    // The opaque sheet under the image: square and without
                    // a shadow, as content is (DESIGN, L1).
                    Rectangle {
                        objectName: "paper"
                        anchors.fill: parent
                        color: pageArea.paper
                        radius: CelestinaTheme.radiusNone
                    }

                    PdfPageImage {
                        anchors.fill: parent
                        document: pageArea.document
                        currentFrame: pageItem.index
                        asynchronous: true
                        fillMode: Image.PreserveAspectFit
                        sourceSize.width: paper.width * Screen.devicePixelRatio
                        sourceSize.height: 0
                    }

                    // Every hit on this page, then the current one and the
                    // pointer's selection in the selection token.
                    Shape {
                        anchors.fill: parent
                        ShapePath {
                            strokeWidth: -1
                            fillColor: CelestinaTheme.accentSoft
                            scale: Qt.size(pageArea.renderScale, pageArea.renderScale)
                            PathMultiline {
                                id: pageHits
                            }
                        }
                    }

                    Connections {
                        target: hits
                        function onCountChanged() { paper.refreshHits() }
                        function onSearchStringChanged() { paper.refreshHits() }
                    }

                    Shape {
                        objectName: "currentHit"
                        anchors.fill: parent
                        visible: hits.currentResult >= 0 && hits.currentPage === pageItem.index
                        ShapePath {
                            strokeWidth: -1
                            fillColor: CelestinaTheme.selectionMarquee
                            scale: Qt.size(pageArea.renderScale, pageArea.renderScale)
                            PathMultiline {
                                paths: hits.currentResultBoundingPolygons
                            }
                        }
                    }

                    Shape {
                        anchors.fill: parent
                        visible: pageArea.selectionPage === pageItem.index
                        ShapePath {
                            strokeWidth: -1
                            fillColor: CelestinaTheme.selectionMarquee
                            scale: Qt.size(pageArea.renderScale, pageArea.renderScale)
                            PathMultiline {
                                paths: selection.geometry
                            }
                        }
                    }

                    // The drag sets both ends itself, so the selection
                    // stays drawn after the button is released.
                    PdfSelection {
                        id: selection
                        anchors.fill: parent
                        document: pageArea.document
                        page: pageItem.index
                        renderScale: pageArea.renderScale
                        onTextChanged: {
                            if (selectionDrag.active) {
                                pageArea.selectionPage = pageItem.index
                                pageArea.selectedText = selection.text
                            }
                        }
                    }

                    DragHandler {
                        id: selectionDrag
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.Stylus
                        target: null
                        cursorShape: Qt.IBeamCursor
                        onActiveChanged: {
                            if (!selectionDrag.active)
                                return
                            if (pageArea.selectionPage !== pageItem.index)
                                pageArea.clearSelection()
                            selection.from = selectionDrag.centroid.pressPosition
                            selection.to = selectionDrag.centroid.position
                        }
                        onCentroidChanged: {
                            if (selectionDrag.active)
                                selection.to = selectionDrag.centroid.position
                        }
                    }

                    TapHandler {
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.Stylus
                        onTapped: pageArea.clearSelection()
                    }

                    Repeater {
                        model: PdfLinkModel {
                            document: pageArea.document
                            page: pageItem.index
                        }

                        delegate: Item {
                            id: linkItem

                            required property int index
                            required property rect rectangle
                            required property url url
                            required property int page
                            required property point location

                            function activate() {
                                if (linkItem.page >= 0)
                                    pageArea.goToLocation(linkItem.page, linkItem.location)
                                else
                                    pageArea.externalLinkRequested(String(linkItem.url))
                            }

                            objectName: "link" + pageItem.index + "_" + linkItem.index
                            x: linkItem.rectangle.x * pageArea.renderScale
                            y: linkItem.rectangle.y * pageArea.renderScale
                            width: linkItem.rectangle.width * pageArea.renderScale
                            height: linkItem.rectangle.height * pageArea.renderScale
                            Accessible.role: Accessible.Link
                            Accessible.name: linkItem.page >= 0
                                             ? qsTr("Ir a la página %1").arg(linkItem.page + 1)
                                             : String(linkItem.url)
                            Accessible.onPressAction: linkItem.activate()

                            HoverHandler {
                                cursorShape: Qt.PointingHandCursor
                            }

                            TapHandler {
                                gesturePolicy: TapHandler.ReleaseWithinBounds
                                onTapped: linkItem.activate()
                            }
                        }
                    }
                }
            }
        }
    }
}
