import QtQuick
import QtQuick.Controls
import org.celestina.grafita 1.0

// Grafita's window: one document, no project, no tabs. The window owns
// activation, the shortcuts and the quit guard; everything below it composes a
// region and nothing more.
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
    title: documentSession.windowTitle

    GrafitaSession {
        id: documentSession

        // The document's text moved underneath the widget — an open, an undo
        // or a redo. Assigning it back is not an edit: the core recognises its
        // own projection and records nothing.
        onDocumentReset: function(text, caret) {
            documentView.adopt(text, caret)
        }

        // A search hit, or a line the user asked to go to.
        onSelectRange: function(start, end) {
            documentView.select(start, end)
        }

        // Only ever emitted once nothing is unsaved, so quitting can never
        // discard an edit.
        onQuitPermitted: {
            window.quitAuthorised = true
            Qt.quit()
        }
    }

    // Once the document has said the window may go, the next close is the one
    // `Qt.quit()` itself asks for and must be accepted. Refusing it too would
    // make the guard fight the exit it just authorised: `Qt.quit()` closes the
    // window, the closing handler refuses, and the two spin forever — which is
    // exactly what happened, leaving a window that only `kill` could end.
    property bool quitAuthorised: false

    // A window close is a request, not a fact: it is refused here and answered
    // once the document says it may go.
    onClosing: function(close) {
        if (window.quitAuthorised) {
            close.accepted = true
            return
        }
        close.accepted = false
        documentSession.requestQuit()
    }

    Shortcut {
        sequences: [StandardKey.Save]
        onActivated: documentSession.save()
    }
    Shortcut {
        sequences: [StandardKey.Undo]
        onActivated: documentSession.undo()
    }
    Shortcut {
        sequences: [StandardKey.Redo]
        onActivated: documentSession.redo()
    }
    Shortcut {
        sequences: [StandardKey.Find]
        onActivated: documentView.openFind()
    }
    Shortcut {
        sequences: [StandardKey.FindNext]
        onActivated: documentSession.findNext()
    }
    Shortcut {
        sequences: [StandardKey.FindPrevious]
        onActivated: documentSession.findPrevious()
    }
    Shortcut {
        sequences: [StandardKey.Replace]
        onActivated: documentView.openFind(true)
    }
    Shortcut {
        sequences: [StandardKey.Close]
        onActivated: documentSession.requestClose()
    }
    Shortcut {
        sequences: [StandardKey.Quit]
        onActivated: documentSession.requestQuit()
    }

    DocumentView {
        id: documentView
        anchors.fill: parent
        session: documentSession
        blocked: documentSession.closePrompt
    }

    UnsavedDialog {
        anchors.fill: parent
        session: documentSession
        backdrop: documentView
    }

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = window.reducedMotion
        if (window.initialPath.length > 0)
            documentSession.openPath(window.initialPath)
    }


}
