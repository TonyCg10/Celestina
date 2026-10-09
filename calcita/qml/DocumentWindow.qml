import QtQuick
import QtQuick.Controls
import QtQuick.Pdf
import org.celestina.calcita 1.0
import "components"

// One document's window: the bar over a continuous page view. QtPdf loads and
// draws the pages; the window's `CalcitaDocument` decides what the page field
// and the zoom controls mean, and the controller keeps the recents. A drop of
// `text/uri-list` opens each PDF in a window of its own.
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
    // This is the window `Activate` raises and that shows the notices no
    // window asked for; the owner sets it.
    property bool front: false

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
                                       readerState.zoomWord())
    }

    function nextPage() {
        if (readerState.page < readerState.pageCount)
            readerState.goTo("+1")
    }

    function previousPage() {
        if (readerState.page > 1)
            readerState.goTo("-1")
    }

    // QtPdf's view moves its current page only when a jump or the scroll
    // bar ends; the page field follows every scroll instead, through the
    // view's own Flickable, found once the view is built.
    property Flickable pageFlick: null

    function findFlick(node) {
        if (node instanceof Flickable)
            return node
        const kids = node.children
        let found = null
        let i = 0
        while (found === null && i < kids.length) {
            found = documentWindow.findFlick(kids[i])
            i += 1
        }
        return found
    }

    // The 1-based page under a line a third down the view, or the last page
    // once the view has reached the end. QtPdf's view stacks the pages with
    // its `rowSpacing`, `pageGap` logical pixels whatever the scale.
    readonly property int pageGap: 6

    function pageAtScroll() {
        const flick = documentWindow.pageFlick
        const count = documentWindow.pdf.pageCount
        if (flick === null || count < 1)
            return 0
        if (flick.contentY >= flick.contentHeight - flick.height - 1)
            return count
        const line = flick.contentY + flick.height / 3
        let top = 0
        let page = 0
        while (page < count - 1) {
            top += documentWindow.pdf.pagePointSize(page).height * pageView.renderScale + documentWindow.pageGap
            if (top > line)
                break
            page += 1
        }
        return page + 1
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

    PdfMultiPageView {
        id: pageView
        objectName: "pageView"
        anchors.fill: parent
        anchors.topMargin: documentBar.height + CelestinaTheme.spaceMd * 2
        document: documentWindow.pdf
        focus: true
        Component.onCompleted: documentWindow.pageFlick = documentWindow.findFlick(pageView)
        onWidthChanged: {
            if (readerState.zoomMode !== "free")
                documentWindow.applyZoom()
        }
        onHeightChanged: {
            if (readerState.zoomMode === "fitPage")
                documentWindow.applyZoom()
        }
    }

    Connections {
        target: documentWindow.pageFlick
        function onContentYChanged() {
            readerState.reportPage(documentWindow.pageAtScroll())
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
        onOpenRequested: documentWindow.openRequested()
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
}
