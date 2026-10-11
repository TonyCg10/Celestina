pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Pdf
import org.celestina.calcita 1.0
import "components"
import "edit"

// One document's window: the bar over a continuous page view, the search card
// under the bar, the outline card at the side and the confirmation an
// external link needs, and the dark reading mode laid over the pages as a
// layer effect. QtPdf loads, draws, searches and selects; the window's
// `CalcitaDocument` decides what the page field, the zoom controls and a step
// through the hits mean, and the controller keeps the recents, the clipboard
// and `xdg-open`. A drop of `text/uri-list` opens each PDF in a window of its
// own.
//
// Editing (Ctrl+E or «Editar»): the window's `CalcitaEditor` reads the
// document and has its pages render from a copy without the editable
// annotations, which each page's `AnnotationLayer` draws instead; the
// bottom `EditToolbar` arms the tools, and leaving or closing with changes
// asks `SaveQuestion`. The reading mode waits while editing. A save that
// kept both lands a copy beside the original, and the window moves to it;
// one that replaced the original reloads it.
//
// Forms: the pages show every widget (from the copy the reader stamps), and
// each page's `FormLayer` lays the fields over it. A tap on a field while
// reading starts editing on it; while editing, text fields and checkboxes
// are controls whose values go to the editor, in the same edit set, undo
// and save as the annotations; Tab follows the form's order across pages.
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
    // Off while editing, which draws over the page as it is.
    property real readingAmount: documentWindow.readingDark && !documentWindow.editing ? 1 : 0
    readonly property alias readingFade: readingFade
    // This is the window `Activate` raises and that shows the notices no
    // window asked for; the owner sets it.
    property bool front: false

    readonly property alias editor: editorState
    readonly property alias editBar: editToolbar
    readonly property alias saveQuestion: question
    readonly property bool editing: editorState.editing
    // The armed tool: `select`, `text`, `highlight` or `ink`.
    property string editTool: "select"
    // The colour new marks take, one of the theme's six accents.
    property string editColorKey: CelestinaTheme.iconAccentKeys[0]
    // The size new marks take: small, medium or large.
    property int editSizeIndex: 1
    // The selected annotation's id, or "".
    property string editSelected: ""
    // Enter asks the selected text box to be edited: its id, for a moment.
    property string editTextOf: ""
    // A text box's field has the keyboard, so the edit keys are its own.
    readonly property bool editTyping: documentWindow.activeFocusItem !== null
                                       && ["draftField", "textBoxField", "formText"].indexOf(
                                           documentWindow.activeFocusItem.objectName) >= 0
    // The text sizes and stroke widths of the three steps, in points.
    readonly property var textSizes: [10, 14, 20]
    readonly property var strokeWidths: [1.5, 3, 6]
    // The form's widgets, from the editor while editing (with the values
    // edited), else from the document.
    readonly property var formFields: JSON.parse(documentWindow.editing ? editorState.fields
                                                                        : readerState.fields)
    // The field (its `order`) to give the keyboard, while `formFocusPending`;
    // each ask changes `formFocusSerial`.
    property int formFocusOrder: -1
    property bool formFocusPending: false
    property int formFocusSerial: 0
    // The window closes once the answer to the save question lands.
    property bool closeAfterAnswer: false
    // The file changed on disk while editing: loaded again once edit mode
    // ends, so the pages, the count, search, selection and links all come
    // from one version of the file.
    property bool reloadPending: false
    // Development only: the smoke's edit was made.
    property bool smokeEdited: false

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

    // Escape answers a waiting link with «Cancelar» first, then clears the
    // edit selection, then closes the search card, then the outline.
    function closeTopCard() {
        if (linkConfirm.asking)
            linkConfirm.cancel()
        else if (documentWindow.editSelected !== "")
            documentWindow.editSelected = ""
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

    // The colour a text or a drawing takes for an accent: a deep ink of its
    // hue, legible on the paper (the theme's emblem ink, made for the same
    // sheet); a highlight takes the accent itself.
    function inkFor(key) {
        return CelestinaTheme.iconEmblemInk(CelestinaTheme.iconAccentColor(key))
    }

    function tintFor(key) {
        return CelestinaTheme.iconAccentColor(key)
    }

    // Ctrl+E and «Editar».
    function toggleEditing() {
        if (editorState.editing)
            documentWindow.leaveEditing()
        else if (!editorState.preparing && readerState.loaded)
            editorState.begin()
    }

    // Leaving asks first when there are changes. The keyboard leaves any
    // text being written first, which adds it.
    function leaveEditing() {
        documentWindow.focusPages()
        if (editorState.edited) {
            documentWindow.closeAfterAnswer = false
            question.shown = true
        } else {
            editorState.finish()
        }
    }

    function selectedItem() {
        const id = documentWindow.editSelected
        const found = JSON.parse(editorState.annotations).find(entry => entry.id === id)
        return found === undefined ? null : found
    }

    // A colour picked: new marks take it, and so does the selection.
    function pickColor(key) {
        documentWindow.editColorKey = key
        const item = documentWindow.selectedItem()
        if (item !== null)
            editorState.recolor(item.id, String(item.kind === "highlight"
                                                ? documentWindow.tintFor(key)
                                                : documentWindow.inkFor(key)))
    }

    function deleteSelected() {
        if (documentWindow.editSelected === "")
            return
        editorState.remove(documentWindow.editSelected)
        documentWindow.editSelected = ""
    }

    // «Resaltar» selected text: QtPdf's outlines of the selection go to the
    // editor, which makes a quadrilateral of each.
    function addHighlight(page, outlines) {
        editorState.addHighlight(page, outlines, String(documentWindow.tintFor(documentWindow.editColorKey)))
    }

    // Gives the field at `order` the keyboard once its control exists.
    function focusField(order) {
        documentWindow.formFocusOrder = order
        documentWindow.formFocusPending = true
        documentWindow.formFocusSerial += 1
    }

    // A tap on a field while reading: editing starts on it.
    function fillFrom(order) {
        documentWindow.focusField(order)
        if (!editorState.editing)
            documentWindow.toggleEditing()
    }

    // Tab or Shift+Tab from the field at `order`: the next field Calcita
    // fills in the form's order, round the ends, brought into view.
    function tabField(order, forward) {
        const fillable = documentWindow.formFields.filter(entry => entry.fillable)
        if (fillable.length === 0)
            return
        const at = fillable.findIndex(entry => entry.order === order)
        const step = forward ? 1 : fillable.length - 1
        const next = fillable[at < 0 ? 0 : (at + step) % fillable.length]
        pageView.reveal(next.page, next.x, next.y, next.w, next.h)
        documentWindow.focusField(next.order)
    }

    // A save landed at `key`: a copy beside the original becomes this
    // window's document, keeping where it was being read; a replaced
    // original is loaded again.
    function landSave(key, replaced) {
        if (replaced) {
            readerState.reload()
        } else if (key !== "" && key !== documentWindow.documentKey) {
            CalcitaController.remember(key, readerState.page, readerState.zoomWord(),
                                       documentWindow.readingDark)
            CalcitaController.renameDocument(documentWindow.documentKey, key)
        }
        if (documentWindow.closeAfterAnswer) {
            documentWindow.closeAfterAnswer = false
            Qt.callLater(() => documentWindow.close())
        }
    }

    // The file is a new version: loaded again where it was being read. An
    // edit in progress keeps the version it began on (its save refuses a
    // changed file); the load waits for edit mode to end.
    function reloadDocument() {
        if (documentWindow.leavingNow)
            return
        if (editorState.editing) {
            documentWindow.reloadPending = true
            return
        }
        documentWindow.reloadPending = false
        documentWindow.remember()
        documentWindow.pdf.source = ""
        documentWindow.pdf.source = readerState.documentUrl
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
    onClosing: close => {
        // Leaving with changes asks first; a save in flight is waited for.
        documentWindow.focusPages()
        if (editorState.saving || editorState.edited) {
            close.accepted = false
            if (!editorState.saving) {
                documentWindow.closeAfterAnswer = true
                question.shown = true
            }
            return
        }
        editorState.finish()
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

    CalcitaEditor {
        id: editorState
        documentKey: documentWindow.documentKey
        onNotice: (kind, text) => chrome.showNotice(kind, text)
        onSaved: (key, replaced) => documentWindow.landSave(key, replaced)
        onSavingChanged: {
            // A save that did not land leaves the window open.
            if (!editorState.saving)
                Qt.callLater(() => documentWindow.closeAfterAnswer = false)
        }
        onEditingChanged: {
            documentWindow.editSelected = ""
            // A save that lands moves to its file itself; whatever is still
            // pending afterwards is loaded now.
            if (!editorState.editing && documentWindow.reloadPending)
                Qt.callLater(() => {
                    if (documentWindow.reloadPending)
                        documentWindow.reloadDocument()
                })
            if (!editorState.editing)
                documentWindow.formFocusPending = false
            if (editorState.editing && CalcitaController.smokeEdit && !documentWindow.smokeEdited) {
                documentWindow.smokeEdited = true
                editorState.addText(0, Qt.point(72, 72), 200, "Calcita smoke",
                                    String(documentWindow.inkFor(documentWindow.editColorKey)), 12)
                editorState.save(false)
            }
            if (editorState.editing && CalcitaController.smokeFill && !documentWindow.smokeEdited) {
                documentWindow.smokeEdited = true
                // The fixture's fields, found by name.
                const field = name => documentWindow.formFields.find(entry => entry.name === name).field
                editorState.fillText(field("name"), "Calcita smoke")
                editorState.setChecked(field("agree"), true)
                editorState.save(false)
            }
        }
        onAnnotationsChanged: {
            if (documentWindow.editSelected !== "" && documentWindow.selectedItem() === null)
                documentWindow.editSelected = ""
        }
    }

    // The document follows this window's key until the window closes; a
    // new key (a copy a save landed) or a new version loads again.
    Component.onCompleted: documentWindow.pdf.source = readerState.documentUrl

    Connections {
        target: readerState
        function onDocumentUrlChanged() {
            // Another file (a copy a save landed): nothing of the old one
            // is pending.
            documentWindow.reloadPending = false
            if (!documentWindow.leavingNow)
                documentWindow.pdf.source = readerState.documentUrl
        }
        function onReloadRequested() {
            documentWindow.reloadDocument()
        }
    }

    // Coming back to the window looks whether the file changed on disk.
    Connections {
        target: documentWindow
        function onActiveChanged() {
            if (documentWindow.active && readerState.loaded)
                readerState.checkFile()
        }
    }

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
                if ((CalcitaController.smokeEdit || CalcitaController.smokeFill)
                        && !documentWindow.smokeEdited)
                    editorState.begin()
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
        anchors.bottomMargin: documentWindow.editing
                              ? editToolbar.height + CelestinaTheme.spaceMd * 2 : 0
        document: documentWindow.pdf
        documentKey: documentWindow.documentKey
        revision: editorState.revision > 0 ? editorState.revision : readerState.revision
        editTool: documentWindow.editing ? documentWindow.editTool : ""
        pageLayer: documentWindow.editing ? layerComponent : null
        formLayer: documentWindow.formFields.length > 0 ? formComponent : null
        onHighlightDragged: (page, outlines) => documentWindow.addHighlight(page, outlines)
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

    // What each page lays over itself while editing.
    Component {
        id: layerComponent

        AnnotationLayer {
            editor: editorState
            tool: documentWindow.editTool
            selectedId: documentWindow.editSelected
            editRequest: documentWindow.editTextOf
            ink: documentWindow.inkFor(documentWindow.editColorKey)
            textSize: documentWindow.textSizes[documentWindow.editSizeIndex]
            strokeWidth: documentWindow.strokeWidths[documentWindow.editSizeIndex]
            onSelectRequested: id => documentWindow.editSelected = id
        }
    }

    // What each page lays over itself when the document has a form.
    Component {
        id: formComponent

        FormLayer {
            fields: documentWindow.formFields
            editor: editorState
            editing: documentWindow.editing
            interactive: !documentWindow.editing || documentWindow.editTool === "select"
                         || documentWindow.editTool === "text"
            focusOrder: documentWindow.formFocusOrder
            focusPending: documentWindow.formFocusPending
            focusSerial: documentWindow.formFocusSerial
            textPadding: editorState.textPadding
            onActivated: order => documentWindow.fillFrom(order)
            onNoticeRequested: text => chrome.showNotice("info", text)
            onTabbed: (order, forward) => documentWindow.tabField(order, forward)
            onFocused: order => {
                documentWindow.formFocusOrder = order
                documentWindow.formFocusPending = false
            }
            onDoneTyping: documentWindow.focusPages()
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
        editing: documentWindow.editing
        onReadingToggled: documentWindow.toggleReadingMode()
        onEditToggled: documentWindow.toggleEditing()
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

    // The edit tools, at the bottom while editing.
    EditToolbar {
        id: editToolbar
        visible: documentWindow.editing
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceMd
        anchors.horizontalCenter: parent.horizontalCenter
        width: Math.min(parent.width - CelestinaTheme.spaceMd * 2, implicitWidth)
        editor: editorState
        tool: documentWindow.editTool
        colorKey: documentWindow.editColorKey
        sizeIndex: documentWindow.editSizeIndex
        hasSelection: documentWindow.editSelected !== ""
        onToolPicked: tool => {
            documentWindow.editTool = tool
            documentWindow.editSelected = ""
        }
        onColorPicked: key => documentWindow.pickColor(key)
        onSizePicked: index => documentWindow.editSizeIndex = index
        onDeleteRequested: documentWindow.deleteSelected()
        onSaveRequested: replace => {
            documentWindow.focusPages()
            editorState.save(replace)
        }
        onDoneRequested: documentWindow.leaveEditing()
    }

    // The drop, the notices this window was given, and Ctrl+O.
    WindowChrome {
        id: chrome
        origin: documentWindow.documentKey
        fallback: documentWindow.front
        noticeLift: documentWindow.editing ? editToolbar.height + CelestinaTheme.spaceMd : 0
        onOpenRequested: documentWindow.openRequested()
    }

    // Leaving or closing with changes: the two outcomes, or «Descartar».
    SaveQuestion {
        id: question
        anchors.fill: parent
        onCopyChosen: {
            question.shown = false
            editorState.save(false)
        }
        onReplaceChosen: {
            question.shown = false
            editorState.save(true)
        }
        onDiscardChosen: {
            question.shown = false
            editorState.finish()
            if (documentWindow.closeAfterAnswer) {
                documentWindow.closeAfterAnswer = false
                Qt.callLater(() => documentWindow.close())
            }
        }
        onDismissRequested: {
            question.shown = false
            documentWindow.closeAfterAnswer = false
        }
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
        enabled: !documentWindow.editing
        onActivated: documentWindow.toggleReadingMode()
    }
    Shortcut {
        sequence: "Ctrl+E"
        onActivated: documentWindow.toggleEditing()
    }
    Shortcut {
        sequence: "Ctrl+Z"
        enabled: documentWindow.editing && !documentWindow.editTyping
        onActivated: editorState.undo()
    }
    Shortcut {
        sequence: "Ctrl+Shift+Z"
        enabled: documentWindow.editing && !documentWindow.editTyping
        onActivated: editorState.redo()
    }
    // Enter edits the selected text box's text.
    Shortcut {
        sequences: ["Return", "Enter"]
        enabled: documentWindow.editing && !documentWindow.editTyping && documentWindow.editSelected !== ""
        onActivated: {
            const item = documentWindow.selectedItem()
            if (item === null || item.kind !== "text")
                return
            documentWindow.editTextOf = item.id
            documentWindow.editTextOf = ""
        }
    }
    // Supr.
    Shortcut {
        sequence: "Delete"
        enabled: documentWindow.editing && !documentWindow.editTyping && documentWindow.editSelected !== ""
        onActivated: documentWindow.deleteSelected()
    }
    Shortcut {
        sequences: [StandardKey.Copy]
        onActivated: documentWindow.copySelection()
    }
    Shortcut {
        sequence: "Escape"
        enabled: linkConfirm.asking || documentWindow.searchOpen || documentWindow.outlineOpen
                 || (documentWindow.editSelected !== "" && !documentWindow.editTyping)
        onActivated: documentWindow.closeTopCard()
    }
}
