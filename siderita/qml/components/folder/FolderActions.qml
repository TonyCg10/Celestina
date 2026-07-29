import QtQuick
import org.celestina.siderita 1.0

Item {
    id: root

    required property var controller
    required property Item owner
    required property Item panel

    property alias sortMenu: folderSortMenu
    property alias entryMenu: entryContextMenu
    property alias pathMenu: breadcrumbMenu
    property alias folderMenu: folderContextMenu
    property alias namePrompt: namePromptDialog
    readonly property bool navigationBlocked:
            folderSortMenu.visible || entryContextMenu.visible
            || breadcrumbMenu.visible || folderContextMenu.visible
            || namePromptDialog.shown || namePromptDialog.visible
            || batchRenameDialog.shown || batchRenameDialog.visible
            || conflictDialog.shown || conflictDialog.visible
            || openWithDialog.shown || openWithDialog.visible
            || propertiesDialog.shown || propertiesDialog.visible
            || iconPickerDialog.shown || iconPickerDialog.visible
            || quickLookDialog.shown || quickLookDialog.visible
            || phoneMediaDialog.shown || phoneMediaDialog.visible

    signal newTabRequested(string path, bool foreground)

    function openPhoneMedia(index) {
        phoneMediaDialog.openPhone(index)
    }

    FolderSortMenu {
        id: folderSortMenu
        backdropSource: root.owner
        controller: root.controller
    }

    EntryContextMenu {
        id: entryContextMenu
        backdropSource: root.owner
        controller: root.controller
        panel: root.panel
        namePrompt: namePromptDialog
        batchRename: batchRenameDialog
        iconPicker: iconPickerDialog
        onNewTabRequested: function(path, foreground) {
            root.newTabRequested(path, foreground)
        }
    }

    PathMenu {
        id: breadcrumbMenu
        backdropSource: root.owner
        controller: root.controller
        onNewTabRequested: function(path, foreground) {
            root.newTabRequested(path, foreground)
        }
    }

    FolderMenu {
        id: folderContextMenu
        backdropSource: root.owner
        controller: root.controller
        panel: root.panel
        namePrompt: namePromptDialog
        onNewTabRequested: function(path, foreground) {
            root.newTabRequested(path, foreground)
        }
    }

    NamePromptDialog {
        id: namePromptDialog
        controller: root.controller
        owner: root.owner
        backdrop: root.panel
    }

    BatchRenameDialog {
        id: batchRenameDialog
        controller: root.controller
        owner: root.owner
        backdrop: root.panel
    }

    ConflictDialog {
        id: conflictDialog
        controller: root.controller
        owner: root.owner
        backdrop: root.panel
    }

    OpenWithDialog {
        id: openWithDialog
        controller: root.controller
        owner: root.owner
        backdrop: root.panel
    }

    PropertiesDialog {
        id: propertiesDialog
        controller: root.controller
        owner: root.owner
        backdrop: root.panel
        panel: root.panel
    }

    IconPickerDialog {
        id: iconPickerDialog
        controller: root.controller
        owner: root.owner
        panel: root.panel
    }

    QuickLookView {
        id: quickLookDialog
        controller: root.controller
        owner: root.owner
        panel: root.panel
    }

    PhoneMediaDialog {
        id: phoneMediaDialog
        controller: root.controller
        owner: root.owner
        backdrop: root.panel
    }
}
