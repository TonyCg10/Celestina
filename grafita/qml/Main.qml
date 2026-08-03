import QtQuick
import QtQuick.Controls
import org.celestina.grafita 1.0

// Grafita's window: one document per tab, no project and no file tree.
//
// Tabs arrived because opening a second file meant a second *window*, which is
// what the author actually hit. Each tab owns its own `GrafitaSession` — the
// same shape Siderita gives each of its tabs a controller — so a document's
// history, dirty state and worker belong to that tab and nothing is shared but
// the window around them.
ApplicationWindow {
    id: window

    required property bool reducedMotion
    // The document named on the command line, already reduced to a local path.
    // Empty means Grafita was launched without one.
    required property string initialPath

    width: 900
    height: 640
    minimumWidth: 480
    minimumHeight: 320
    visible: true
    color: CelestinaTheme.canvas
    title: activeSession ? activeSession.windowTitle : "Grafita"

    // ── Tabs ─────────────────────────────────────────────────────────────
    property int currentTab: 0
    // Bumped whenever a tab is added or removed, so bindings that reach through
    // `itemAt()` re-resolve once the delegate exists.
    property int tabsRevision: 0

    readonly property var activeSession: {
        tabsRevision
        const holder = tabRepeater.itemAt(currentTab)
        return holder ? holder.tabSession : null
    }
    readonly property var activeView: {
        tabsRevision
        const holder = tabRepeater.itemAt(currentTab)
        return holder ? holder.tabView : null
    }

    ListModel { id: tabsModel }

    /// Opens `path` in a new tab, or focuses the tab that already has it —
    /// asking twice for the same document should not give you it twice.
    function openTab(path) {
        const wanted = path === undefined || path === null ? "" : path
        if (wanted.length > 0) {
            for (let index = 0; index < tabsModel.count; ++index) {
                const holder = tabRepeater.itemAt(index)
                if (holder && holder.tabSession && holder.tabSession.path === wanted) {
                    window.currentTab = index
                    window.requestActivate()
                    return
                }
            }
        }
        tabsModel.append({ initialPath: wanted })
        window.currentTab = tabsModel.count - 1
    }

    /// Closes a tab through its document, so unsaved work still stops it. The
    /// tab goes away when the session says its document did.
    function requestCloseTab(index) {
        const holder = tabRepeater.itemAt(index)
        if (!holder || !holder.tabSession)
            return
        window.currentTab = index
        holder.tabSession.requestClose()
    }

    function dropTab(index) {
        if (index < 0 || index >= tabsModel.count)
            return
        tabsModel.remove(index)
        if (tabsModel.count === 0) {
            // The last document closing leaves an empty tab rather than an
            // empty window with no way back in: the empty state has the button.
            tabsModel.append({ initialPath: "" })
            window.currentTab = 0
            return
        }
        window.currentTab = Math.min(index, tabsModel.count - 1)
    }

    function cycleTab(delta) {
        if (tabsModel.count <= 1)
            return
        window.currentTab =
            (window.currentTab + delta + tabsModel.count) % tabsModel.count
    }

    function sessionAt(index) {
        const holder = tabRepeater.itemAt(index)
        return holder ? holder.tabSession : null
    }

    /// Moves a dragged tab from `from` to `to`, keeping the *active* tab
    /// pointed at correctly even when some other tab is the one that moved.
    ///
    /// `currentTab` is an index, but ListModel.move() reuses delegate
    /// instances rather than recreating them, so the fix is to remember which
    /// delegate object was current, move, and then find where that same
    /// object ended up.
    function reorderTab(from, to) {
        if (from === to || from < 0 || to < 0
                || from >= tabsModel.count || to >= tabsModel.count)
            return
        const currentHolder = tabRepeater.itemAt(window.currentTab)
        tabsModel.move(from, to, 1)
        for (let index = 0; index < tabsModel.count; ++index) {
            if (tabRepeater.itemAt(index) === currentHolder) {
                window.currentTab = index
                break
            }
        }
    }

    // ── Quitting ─────────────────────────────────────────────────────────
    // Once every document has said the window may go, the next close is the one
    // `Qt.quit()` itself asks for and must be accepted. Refusing it too would
    // make the guard fight the exit it just authorised — the two would spin
    // forever, which is exactly what happened before this flag existed.
    property bool quitAuthorised: false
    // Which tab the quit sweep is asking. Quitting has to walk every dirty
    // document, not just the visible one, or closing the window would discard
    // work the user never saw asked about.
    property int quitCursor: -1

    function requestQuit() {
        window.quitCursor = 0
        window.continueQuit()
    }

    function continueQuit() {
        while (window.quitCursor < tabsModel.count) {
            const pending = window.sessionAt(window.quitCursor)
            if (pending && pending.active) {
                window.currentTab = window.quitCursor
                pending.requestQuit()
                return
            }
            window.quitCursor += 1
        }
        window.quitAuthorised = true
        Qt.quit()
    }

    function cancelQuit() {
        window.quitCursor = -1
    }

    onClosing: function(close) {
        if (window.quitAuthorised) {
            close.accepted = true
            return
        }
        close.accepted = false
        window.requestQuit()
    }

    Shortcut {
        sequences: [StandardKey.Save]
        onActivated: if (activeSession) activeSession.save()
    }
    Shortcut {
        sequences: [StandardKey.Undo]
        onActivated: if (activeSession) activeSession.undo()
    }
    Shortcut {
        sequences: [StandardKey.Redo]
        onActivated: if (activeSession) activeSession.redo()
    }
    Shortcut {
        sequences: [StandardKey.Find]
        onActivated: if (activeView) activeView.openFind()
    }
    Shortcut {
        sequences: [StandardKey.FindNext]
        onActivated: if (activeSession) activeSession.findNext()
    }
    Shortcut {
        sequences: [StandardKey.FindPrevious]
        onActivated: if (activeSession) activeSession.findPrevious()
    }
    Shortcut {
        sequences: [StandardKey.Replace]
        onActivated: if (activeView) activeView.openFind(true)
    }
    Shortcut {
        sequences: [StandardKey.AddTab]
        onActivated: window.openTab("")
    }
    Shortcut {
        sequences: [StandardKey.Close]
        onActivated: window.requestCloseTab(window.currentTab)
    }
    Shortcut {
        sequences: [StandardKey.NextChild]
        onActivated: window.cycleTab(1)
    }
    Shortcut {
        sequences: [StandardKey.PreviousChild]
        onActivated: window.cycleTab(-1)
    }
    Shortcut {
        sequences: [StandardKey.Quit]
        onActivated: window.requestQuit()
    }

    TabStrip {
        id: tabStrip
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        tabs: tabsModel
        current: window.currentTab
        revision: window.tabsRevision
        titleFor: function(index) {
            const tab = window.sessionAt(index)
            return tab && tab.name.length > 0 ? tab.name : "Sin título"
        }
        dirtyFor: function(index) {
            const tab = window.sessionAt(index)
            return tab ? tab.dirty : false
        }
        onSelected: function(index) { window.currentTab = index }
        onCloseRequested: function(index) { window.requestCloseTab(index) }
        onNewRequested: window.openTab("")
        onReorderRequested: function(from, to) { window.reorderTab(from, to) }
    }

    Item {
        id: tabArea
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: tabStrip.bottom
        anchors.bottom: parent.bottom

        Repeater {
            id: tabRepeater
            model: tabsModel
            onCountChanged: window.tabsRevision += 1

            delegate: Item {
                id: holder
                required property string initialPath
                required property int index

                property alias tabSession: documentSession
                property alias tabView: view

                anchors.fill: parent
                visible: holder.index === window.currentTab
                enabled: visible

                GrafitaSession {
                    id: documentSession

                    onDocumentReset: function(text, caret) {
                        view.adopt(text, caret)
                    }
                    onSelectRange: function(start, end) {
                        view.select(start, end)
                    }
                    // Nowhere to save to yet: ask, then come back through
                    // `saveAs`. Asked here rather than up front, because a
                    // document only needs a name when it is being kept.
                    onDestinationNeeded: view.askDestination()
                    // The tab's document said its state changed; the strip
                    // shows names and dirty marks that live here, not in the
                    // model, so it is told to re-read them.
                    onNameChanged: window.tabsRevision += 1
                    onDirtyChanged: window.tabsRevision += 1

                    // The document is gone. Outside a quit sweep that means the
                    // user closed this tab, so the tab goes with it. During a
                    // sweep the tabs must stay put until it finishes, or
                    // removing one would shift the indices it is walking.
                    onClosed: if (window.quitCursor < 0)
                                  window.dropTab(holder.index)

                    // Nothing unsaved is left. During a quit sweep that is the
                    // cue to ask the next tab.
                    onQuitPermitted: if (window.quitCursor >= 0) {
                        window.quitCursor += 1
                        window.continueQuit()
                    }
                }

                DocumentView {
                    id: view
                    anchors.fill: parent
                    session: documentSession
                    blocked: documentSession.closePrompt
                }

                UnsavedDialog {
                    anchors.fill: parent
                    session: documentSession
                    backdrop: view
                    onCancelled: window.cancelQuit()
                }

                Component.onCompleted: if (holder.initialPath.length > 0)
                                           documentSession.openPath(holder.initialPath)
            }
        }
    }

    // A second `grafita RUTA` hands its document here instead of mapping another
    // window. Best-effort: without a session bus this simply never fires and
    // every launch opens its own window, which is where Grafita started.
    GrafitaActivation {
        id: activation

        onOpenRequested: function(path) {
            window.openTab(path)
            window.requestActivate()
        }
    }

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = window.reducedMotion
        // Always one tab, even with no document: the empty state carries the
        // button that opens one.
        window.openTab(window.initialPath)
        activation.start()
    }





}
