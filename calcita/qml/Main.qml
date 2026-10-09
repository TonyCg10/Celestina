pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Pdf
import org.celestina.calcita 1.0
import "components"

// Calcita's first window: the empty state while no document is open. Every
// admitted document opens a `DocumentWindow` of its own (one document per
// window, all in this process); once the first one opens, this window
// retires for the rest of the session, so closing the last document ends the
// process. Activation, the file chooser and the drop all end in the
// controller, which says which documents open.
ApplicationWindow {
    id: window

    // The activation adapter, the open windows and the chooser, for the
    // route below and for the tests.
    readonly property alias activation: activationAdapter
    readonly property alias documentWindows: documentInstantiator
    readonly property alias fileDialog: chooser
    // A document has opened: this window stays hidden from then on.
    property bool retired: false
    // The document window that last had the focus, while it is open.
    property var lastActive: null
    // The window `Activate` brings forward, that parents the file chooser and
    // that shows the notices no window asked for: the document window last
    // focused, else the newest, else this one.
    readonly property var front: window.pickFront(window.lastActive, documentInstantiator.count,
                                                  window.leavingTick)
    // Bumped when a window starts closing, so `front` looks again.
    property int leavingTick: 0
    // The window whose chooser is open: a refusal of its result goes there.
    property string chooserOrigin: "main"

    // The front window: `last` if set, else the newest document window that
    // is not closing, else this one. `count` and `tick` are what the binding
    // depends on.
    function pickFront(last, count, tick) {
        if (last !== null)
            return last
        let row = count - 1
        while (row >= 0 && documentInstantiator.objectAt(row).leavingNow)
            row -= 1
        return row >= 0 ? documentInstantiator.objectAt(row) : window
    }

    // Opens the file chooser over the front window, for `origin`.
    function openChooser(origin) {
        window.chooserOrigin = origin
        // A visible transient parent: the retired first window is hidden.
        // Set when opening, not bound, so the dialog never holds on to a
        // document window after it closes.
        chooser.parentWindow = window.front
        chooser.open()
    }

    // `closing` is closing: it stops being the front window or the chooser's
    // parent at once (the controller already forgot its document).
    function forgetWindow(closing) {
        if (window.lastActive === closing)
            window.lastActive = null
        if (chooser.parentWindow === closing)
            chooser.parentWindow = null
        window.leavingTick += 1
    }

    // The QtPdf documents, one per window ever open at once. They are never
    // destroyed while Calcita runs: QtPdf's image reader thread may still be
    // reading a page of a closed window's document, so a closed window's
    // document is emptied and kept for the next window instead.
    property var pdfDocuments: []
    property var sparePdfDocuments: []

    function takePdfDocument() {
        const spare = window.sparePdfDocuments
        if (spare.length > 0) {
            window.sparePdfDocuments = spare.slice(0, spare.length - 1)
            return spare[spare.length - 1]
        }
        const created = pdfComponent.createObject(window)
        window.pdfDocuments = window.pdfDocuments.concat([created])
        return created
    }

    // `pdf` was emptied by its window when it closed.
    function givePdfDocumentBack(pdf) {
        window.sparePdfDocuments = window.sparePdfDocuments.concat([pdf])
    }

    // A turn after the window of `key` closed: its row, the oldest one for
    // `key`, leaves the list and the window is destroyed.
    function dropDocument(key) {
        const row = window.rowOf(key)
        if (row >= 0)
            openDocuments.remove(row)
    }

    function rowOf(key) {
        const rows = openDocuments.count
        let row = 0
        while (row < rows && openDocuments.get(row).key !== key)
            row += 1
        return row < rows ? row : -1
    }

    width: 900
    height: 680
    minimumWidth: 480
    minimumHeight: 360
    visible: !window.retired
    // Transparent: the compositor blurs what lies behind the window and
    // CelestinaBackdrop lays the canvas over it (DESIGN §5.2 L0).
    color: CelestinaTheme.clear
    title: "Calcita"

    CelestinaBackdrop {
        anchors.fill: parent
    }

    // The suite's appearance file (reduced motion, text scale), followed by
    // the controller and bound into the theme once for every window.
    CelestinaAppearance {
        reducedMotion: CalcitaController.appearanceReducedMotion
        textScale: CalcitaController.appearanceTextScale
    }

    // A second launch brings the front window forward or opens documents.
    CalcitaActivation {
        id: activationAdapter
    }

    ActivationRoute {
        source: activationAdapter
        host: window.front
    }

    Component {
        id: pdfComponent
        PdfDocument {}
    }

    // The documents open now, one window each, in the order they opened.
    ListModel {
        id: openDocuments
    }

    Instantiator {
        id: documentInstantiator
        model: openDocuments

        delegate: DocumentWindow {
            id: documentWindow
            required property string key
            required property int slot
            documentKey: key
            pdf: window.pdfDocuments[slot]
            front: window.front === documentWindow
            onActiveChanged: {
                if (documentWindow.active)
                    window.lastActive = documentWindow
            }
            onOpenRequested: window.openChooser(documentWindow.documentKey)
            onLeaving: window.forgetWindow(documentWindow)
            onClosed: closedKey => {
                window.givePdfDocumentBack(documentWindow.pdf)
                window.dropDocument(closedKey)
            }
        }
    }

    Connections {
        target: CalcitaController
        function onOpenDocument(key) {
            const pdf = window.takePdfDocument()
            openDocuments.append({ "key": key, "slot": window.pdfDocuments.indexOf(pdf) })
            window.retired = true
        }
    }

    // The FileChooser portal (Siderita's backend), through Qt's dialog.
    FileDialog {
        id: chooser
        title: qsTr("Abrir un PDF")
        fileMode: FileDialog.OpenFiles
        nameFilters: [qsTr("Documentos PDF (*.pdf *.PDF)")]
        onAccepted: CalcitaController.openDropped(chooser.selectedFiles.map(url => url.toString()),
                                                  window.chooserOrigin)
    }

    EmptyState {
        id: emptyState
        anchors.centerIn: parent
        width: Math.min(parent.width - CelestinaTheme.windowMargin * 2, 360)
        recentKeys: CalcitaController.recents
        recentNames: CalcitaController.recentNames
        onOpenRequested: window.openChooser("main")
        onRecentChosen: key => CalcitaController.openPath(key, "main")
    }

    // The drop, the notices this window was given (and those no window
    // asked for while it is in front), and Ctrl+O.
    WindowChrome {
        origin: "main"
        fallback: window.front === window
        onOpenRequested: window.openChooser("main")
    }

    // Development only: the smoke reads what the windows show once they are
    // up.
    Timer {
        interval: 3000
        running: CalcitaController.smokeReport
        onTriggered: {
            const document = window.front === window ? null : window.front
            console.log("calcita-smoke:"
                        + " empty=" + window.visible
                        + " pageCount=" + (document ? document.reader.pageCount : 0)
                        + " textScale=" + CelestinaTheme.textScale
                        + " fontBody=" + CelestinaTheme.fontBody)
        }
    }

    Component.onCompleted: activationAdapter.start()
}
