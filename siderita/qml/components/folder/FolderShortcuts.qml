import QtQuick

Item {
    id: root

    required property bool viewActive
    required property bool canGoBackOrLeave
    required property var controller
    required property var panel
    required property var topBar
    required property var namePrompt

    signal goBackOrLeaveRequested

    Shortcut {
        sequence: "Alt+Left"
        enabled: root.viewActive && root.canGoBackOrLeave
        onActivated: root.goBackOrLeaveRequested()
    }

    Shortcut {
        sequence: "Alt+Right"
        enabled: root.viewActive && root.controller.canGoForward && !root.controller.loading
        onActivated: root.controller.goForward()
    }

    Shortcut {
        sequence: "Alt+Up"
        enabled: root.viewActive && root.controller.canGoUp && !root.controller.loading
        onActivated: root.controller.goUp()
    }

    Shortcut {
        sequence: "Ctrl+L"
        enabled: root.viewActive
        onActivated: root.topBar.beginEditing()
    }

    Shortcut {
        sequence: "Ctrl+F"
        enabled: root.viewActive
        onActivated: root.topBar.focusSearch()
    }

    Shortcut {
        sequence: "Ctrl+H"
        enabled: root.viewActive && !root.controller.loading
        onActivated: root.controller.toggleHidden()
    }

    Shortcut {
        sequence: "F5"
        enabled: root.viewActive && !root.controller.loading
        onActivated: root.controller.refresh()
    }

    Shortcut {
        sequence: "F2"
        enabled: root.viewActive && !root.controller.loading
        onActivated: {
            const index = root.topBar.activeView.currentIndex
            if (index >= 0)
                root.namePrompt.openRename(root.controller.entryPath(index),
                                           root.controller.entryNames[index])
        }
    }

    Shortcut {
        sequence: "Delete"
        enabled: root.viewActive && !root.controller.loading
                 && !root.controller.trashActive
        onActivated: {
            const paths = root.panel.selectedPaths()
            if (paths.length > 1)
                root.controller.trashPaths(paths)
            else if (paths.length === 1)
                root.controller.trashPath(paths[0])
            else {
                const index = root.topBar.activeView.currentIndex
                if (index >= 0)
                    root.controller.trashPath(root.controller.entryPath(index))
            }
        }
    }

    Shortcut {
        sequences: [StandardKey.Copy]
        enabled: root.viewActive && !root.controller.trashActive
        onActivated: root.copySelection(false)
    }

    Shortcut {
        sequences: [StandardKey.Cut]
        enabled: root.viewActive && !root.controller.trashActive
        onActivated: root.copySelection(true)
    }

    Shortcut {
        sequences: [StandardKey.Paste]
        // Not gated on `canPaste`: that property is a *menu* state, refreshed
        // when the folder menu opens, so it stays false after copying in
        // another application — or in another tab, since the internal clipboard
        // belongs to a controller. The shortcut would then do nothing at all,
        // with no way for a person to tell why.
        //
        // `paste()` already reads the system clipboard itself and does nothing
        // when there is nothing to paste, so asking it is both cheaper and
        // truer than asking a cached answer.
        enabled: root.viewActive && !root.controller.trashActive
        onActivated: root.controller.paste()
    }

    Shortcut {
        sequences: [StandardKey.Undo]
        // Undo stays out while anything is writing: the record it would reverse
        // is the last finished write, and a running one is about to replace it.
        enabled: root.viewActive && root.controller.canUndo
                 && !root.controller.loading && !root.controller.opRunning
        onActivated: root.controller.undo()
    }

    function copySelection(cut) {
        const paths = panel.selectedPaths()
        if (paths.length > 1)
            controller.copyPathsToClipboard(paths, cut)
        else if (paths.length === 1)
            controller.copyToClipboard(paths[0], cut)
        else {
            const index = topBar.activeView.currentIndex
            if (index >= 0)
                controller.copyToClipboard(controller.entryPath(index), cut)
        }
    }

}
