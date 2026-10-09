pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Pdf
import org.celestina.calcita 1.0
import "components"

// One document's window: the bar over a continuous page view, the search card
// under the bar, the outline card at the side and the confirmation an
// external link needs, and the dark reading mode laid over the pages as a
// layer effect. QtPdf loads, draws, searches and selects; the window's
// `CalcitaDocument` decides what the page field, the zoom controls and a step
// through the hits mean, and the controller keeps the recents, the clipboard
// and `xdg-open`. A drop of `text/uri-list` opens each PDF in a window of its
// own.
ApplicationWindow {
    id: documentWindow

    // The pathkey of the document this window shows.
    required property string documentKey
    // The QtPdf document this window reads through. The owner keeps it and
    // it outlives the window: QtPdf renders on Qt's image reader thread,
    // which may still be reading a page after the window is gone, so the
    // document must never be destroyed with the window.
    required property PdfDocument pdf
    // The window is closing and waits for the owner to destroy it.
    property bool leavingNow: false

    // The window is closing: the owner stops treating it as a live window
    // (front, chooser parent) at once.
    signal leaving()
    // A turn after closing: the owner may now destroy the window.
    signal closed(string key)
    // «Abrir…» or Ctrl+O: the owner shows the file chooser.
    signal openRequested()

    readonly property alias reader: readerState
    readonly property alias view: pageView
    readonly property alias bar: documentBar
    readonly property alias chrome: chrome
    readonly property alias search: searchCard
    readonly property alias outline: outlinePanel
    readonly property alias linkPill: linkConfirm
    property bool searchOpen: false
    property bool outlineOpen: false
    // The dark reading mode (Ctrl+I or the bar's button): the pages inverted
    // with their hue turned, remembered per document.
    property bool readingDark: false
    // The reading mode's strength, 0 to 1: it fades over `motionNormal`, at
    // once under reduced motion. The layer exists only while it is above 0.
    property real readingAmount: documentWindow.readingDark ? 1 : 0
    readonly property alias readingFade: readingFade
    // This is the window `Activate` raises and that shows the notices no
    // window asked for; the owner sets it.
    property bool front: false

    Behavior on readingAmount {
        id: readingFade
        enabled: !CelestinaTheme.reducedMotion
        NumberAnimation {
            duration: CelestinaTheme.motionNormal
            easing.type: CelestinaTheme.easeStandard
        }
    }

    // The pages sit this far from the window's sides when fitted.
    readonly property int fitMargin: CelestinaTheme.spaceXl

    function applyZoom() {
        if (!readerState.loaded)
            return
        const width = Math.max(1, pageView.width - documentWindow.fitMargin)
        const height = Math.max(1, pageView.height - documentWindow.fitMargin)
        if (readerState.zoomMode === "fitWidth") {
            pageView.scaleToWidth(width, height)
            readerState.reportScale(pageView.renderScale)
        } else if (readerState.zoomMode === "fitPage") {
            pageView.scaleToPage(width, height)
            readerState.reportScale(pageView.renderScale)
        } else {
            pageView.renderScale = readerState.zoomFactor
        }
    }

    function remember() {
        if (readerState.loaded)
            CalcitaController.remember(documentWindow.documentKey, readerState.page,
                                       readerState.zoomWord(), documentWindow.readingDark)
    }

    function toggleReadingMode() {
        documentWindow.readingDark = !documentWindow.readingDark
        documentWindow.remember()
    }

    function nextPage() {
        if (readerState.page < readerState.pageCount)
            readerState.goTo("+1")
    }

    function previousPage() {
        if (readerState.page > 1)
            readerState.goTo("-1")
    }

    // The 1-based page under a line a third down the view, or the last page
    // once the view has reached the end.
    function pageAtScroll() {
        const count = pageView.count
        if (count < 1)
            return 0
        if (pageView.contentY >= pageView.originY + pageView.contentHeight - pageView.height - 1)
            return count
        const page = pageView.pageAt(pageView.contentY + pageView.height / 3)
        return page >= 0 ? page + 1 : 0
    }

    function openSearch() {
        documentWindow.searchOpen = true
        searchCard.focusField()
    }

    function closeSearch() {
        documentWindow.searchOpen = false
        searchCard.reset()
        pageView.searchModel.searchString = ""
        documentWindow.focusPages()
    }

    // Enter, F3 and the next button (`forward`), or their Shift forms.
    function stepHit(forward) {
        const hits = pageView.searchModel
        if (hits.count < 1)
            return
        pageView.showHit(readerState.nextHit(hits.currentResult, hits.count, forward))
    }

    function toggleOutline() {
        documentWindow.outlineOpen = !documentWindow.outlineOpen
        if (documentWindow.outlineOpen)
            outlinePanel.focusTree()
        else
            documentWindow.focusPages()
    }

    // Escape answers a waiting link with «Cancelar» first, then closes the
    // search card, then the outline.
    function closeTopCard() {
        if (linkConfirm.asking)
            linkConfirm.cancel()
        else if (documentWindow.searchOpen)
            documentWindow.closeSearch()
        else if (documentWindow.outlineOpen)
            documentWindow.toggleOutline()
    }

    function copySelection() {
        if (pageView.selectedText.length > 0)
            CalcitaController.copySelection(pageView.selectedText)
    }

    function focusPages() {
        pageView.forceActiveFocus()
    }

    width: 900
    height: 760
    minimumWidth: 480
    minimumHeight: 360
    visible: true
    color: CelestinaTheme.clear
    title: readerState.documentName !== "" ? readerState.documentName : "Calcita"

    // Closing is safe by design: the document belongs to the owner and is
    // only emptied here (QtPdf's image reader thread may still be reading
    // one of its pages), and the owner, which destroys this window and takes
    // the document back, hears of the close on the next event-loop turn.
    // The controller forgets the document at once, so an `Open` of the same
    // path in between opens a fresh window instead of raising this one.
    onClosing: {
        documentWindow.remember()
        documentWindow.leavingNow = true
        documentWindow.pdf.source = ""
        const key = documentWindow.documentKey
        CalcitaController.closeDocument(key)
        documentWindow.leaving()
        Qt.callLater(() => documentWindow.closed(key))
    }

    CelestinaBackdrop {
        anchors.fill: parent
    }

    CalcitaDocument {
        id: readerState
        documentKey: documentWindow.documentKey
        onPageRequested: page => pageView.goToPage(page - 1)
        onNotice: (kind, text) => chrome.showNotice(kind, text)
        onZoomModeChanged: documentWindow.applyZoom()
        onZoomFactorChanged: {
            if (readerState.zoomMode === "free")
                documentWindow.applyZoom()
        }
    }

    // The document follows this window's key until the window closes.
    Component.onCompleted: documentWindow.pdf.source = readerState.documentUrl

    Connections {
        target: documentWindow.pdf
        function onStatusChanged() {
            const pdf = documentWindow.pdf
            if (pdf.status === PdfDocument.Ready) {
                readerState.reportLoaded(pdf.pageCount)
                readerState.restoreZoom(CalcitaController.restoredZoom(documentWindow.documentKey))
                documentWindow.readingDark = CalcitaController.restoredDark(documentWindow.documentKey)
                documentWindow.applyZoom()
                const page = CalcitaController.restoredPage(documentWindow.documentKey)
                if (page > 1 && page <= pdf.pageCount)
                    readerState.goTo(String(page))
                documentWindow.remember()
            } else if (pdf.status === PdfDocument.Error) {
                readerState.reportLoaded(-1)
                chrome.showNotice("error", qsTr("No se pudo abrir el documento."))
            }
        }
    }

    PageView {
        id: pageView
        anchors.fill: parent
        anchors.topMargin: documentBar.height + CelestinaTheme.spaceMd * 2
        anchors.leftMargin: documentWindow.outlineOpen
                            ? outlinePanel.width + CelestinaTheme.spaceMd * 2 : 0
        document: documentWindow.pdf
        focus: true
        layer.enabled: documentWindow.readingAmount > 0
        layer.effect: ReadingEffect {
            amount: documentWindow.readingAmount
        }
        onExternalLinkRequested: url => linkConfirm.ask(url)
        onContentYChanged: readerState.reportPage(documentWindow.pageAtScroll())
        onWidthChanged: {
            if (readerState.zoomMode !== "free")
                documentWindow.applyZoom()
        }
        onHeightChanged: {
            if (readerState.zoomMode === "fitPage")
                documentWindow.applyZoom()
        }
    }

    CelestinaScrollBar {
        surface: pageView
        anchors.right: pageView.right
        anchors.top: pageView.top
        anchors.bottom: pageView.bottom
    }

    OutlinePanel {
        id: outlinePanel
        visible: documentWindow.outlineOpen
        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.top: pageView.top
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceMd
        width: 260
        document: documentWindow.pdf
        onBookmarkChosen: (page, location) => {
            pageView.goToLocation(page, location)
            readerState.reportPage(page + 1)
        }
        onCloseRequested: documentWindow.toggleOutline()
    }

    SearchCard {
        id: searchCard
        visible: documentWindow.searchOpen
        anchors.top: documentBar.bottom
        anchors.topMargin: CelestinaTheme.spaceSm
        anchors.horizontalCenter: parent.horizontalCenter
        width: Math.min(parent.width - CelestinaTheme.windowMargin * 2, implicitWidth)
        hitCount: pageView.searchModel.count
        currentHit: pageView.searchModel.currentResult
        searching: pageView.searchModel.searchString.length > 0
        onQueryEdited: text => pageView.searchModel.searchString = readerState.searchQuery(text)
        onNextRequested: documentWindow.stepHit(true)
        onPreviousRequested: documentWindow.stepHit(false)
        onCloseRequested: documentWindow.closeSearch()
    }

    LinkPill {
        id: linkConfirm
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceXl
        anchors.horizontalCenter: parent.horizontalCenter
        onConfirmed: url => {
            CalcitaController.openExternal(url, documentWindow.documentKey)
            documentWindow.focusPages()
        }
    }

    CelestinaSurface {
        objectName: "loadError"
        visible: documentWindow.pdf.status === PdfDocument.Error
        anchors.centerIn: parent
        width: Math.min(parent.width - CelestinaTheme.windowMargin * 2, 360)
        height: errorText.implicitHeight + CelestinaTheme.spaceLg * 2
        role: CelestinaSurface.Grouped

        Text {
            id: errorText
            x: CelestinaTheme.spaceLg
            y: CelestinaTheme.spaceLg
            width: parent.width - CelestinaTheme.spaceLg * 2
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: qsTr("No se pudo abrir el documento.")
            color: CelestinaTheme.text
            font.pixelSize: CelestinaTheme.fontBody
        }
    }

    DocumentBar {
        id: documentBar
        anchors.top: parent.top
        anchors.topMargin: CelestinaTheme.spaceMd
        anchors.horizontalCenter: parent.horizontalCenter
        name: readerState.documentName
        page: readerState.page
        pageCount: readerState.pageCount
        zoomMode: readerState.zoomMode
        zoomFactor: readerState.zoomFactor
        onGoToRequested: text => readerState.goTo(text)
        onZoomInRequested: readerState.zoomIn()
        onZoomOutRequested: readerState.zoomOut()
        onFitWidthRequested: readerState.fitWidth()
        onFitPageRequested: readerState.fitPage()
        searchOpen: documentWindow.searchOpen
        outlineOpen: documentWindow.outlineOpen
        readingDark: documentWindow.readingDark
        onReadingToggled: documentWindow.toggleReadingMode()
        onOpenRequested: documentWindow.openRequested()
        onSearchToggled: {
            if (documentWindow.searchOpen)
                documentWindow.closeSearch()
            else
                documentWindow.openSearch()
        }
        onOutlineToggled: documentWindow.toggleOutline()
        onFieldDone: documentWindow.focusPages()
    }

    // An `Open` of a document already shown brings this window forward.
    Connections {
        target: CalcitaController
        function onRaiseDocument(key) {
            if (key === documentWindow.documentKey) {
                documentWindow.show()
                documentWindow.raise()
                documentWindow.requestActivate()
            }
        }
    }

    // The drop, the notices this window was given, and Ctrl+O.
    WindowChrome {
        id: chrome
        origin: documentWindow.documentKey
        fallback: documentWindow.front
        onOpenRequested: documentWindow.openRequested()
    }

    // The keyboard (DESIGN: every action has a key).
    Shortcut {
        sequences: [StandardKey.MoveToNextPage, "Space"]
        onActivated: documentWindow.nextPage()
    }
    Shortcut {
        sequences: [StandardKey.MoveToPreviousPage, "Shift+Space"]
        onActivated: documentWindow.previousPage()
    }
    Shortcut {
        sequences: [StandardKey.MoveToStartOfDocument, "Home"]
        onActivated: readerState.goTo("inicio")
    }
    Shortcut {
        sequences: [StandardKey.MoveToEndOfDocument, "End"]
        onActivated: readerState.goTo("fin")
    }
    Shortcut {
        sequence: "Ctrl+G"
        onActivated: documentBar.focusPageField()
    }
    Shortcut {
        sequences: [StandardKey.ZoomIn, "Ctrl+="]
        onActivated: readerState.zoomIn()
    }
    Shortcut {
        sequences: [StandardKey.ZoomOut]
        onActivated: readerState.zoomOut()
    }
    Shortcut {
        sequence: "Ctrl+0"
        onActivated: readerState.setZoom(1)
    }
    Shortcut {
        sequence: "Ctrl+1"
        onActivated: readerState.fitWidth()
    }
    Shortcut {
        sequence: "Ctrl+2"
        onActivated: readerState.fitPage()
    }
    Shortcut {
        sequences: [StandardKey.Find]
        onActivated: documentWindow.openSearch()
    }
    Shortcut {
        sequence: "F3"
        onActivated: documentWindow.stepHit(true)
    }
    Shortcut {
        sequence: "Shift+F3"
        onActivated: documentWindow.stepHit(false)
    }
    // The outline, F9 as in other readers' side pane.
    Shortcut {
        sequence: "F9"
        onActivated: documentWindow.toggleOutline()
    }
    Shortcut {
        sequence: "Ctrl+I"
        onActivated: documentWindow.toggleReadingMode()
    }
    Shortcut {
        sequences: [StandardKey.Copy]
        onActivated: documentWindow.copySelection()
    }
    Shortcut {
        sequence: "Escape"
        enabled: linkConfirm.asking || documentWindow.searchOpen || documentWindow.outlineOpen
        onActivated: documentWindow.closeTopCard()
    }
}
