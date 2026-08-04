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
    readonly property bool modalBlocked:
            namePromptDialog.shown || namePromptDialog.visible
            || batchRenameDialog.shown || batchRenameDialog.visible
            || conflictDialog.shown || conflictDialog.visible
            || openWithDialog.shown || openWithDialog.visible
            || propertiesDialog.shown || propertiesDialog.visible
            || iconPickerDialog.shown || iconPickerDialog.visible
            || quickLookDialog.shown || quickLookDialog.visible
            || grafitaEditorDialog.shown || grafitaEditorDialog.visible
            || phoneMediaDialog.shown || phoneMediaDialog.visible
    readonly property bool navigationBlocked:
            folderSortMenu.visible || entryContextMenu.visible
            || breadcrumbMenu.visible || folderContextMenu.visible
            || modalBlocked

    signal newTabRequested(string path, bool foreground)

    function openPhoneMedia(index) {
        phoneMediaDialog.openPhone(index)
    }

    // Space asks the document core — by content, never by filename — whether
    // the selected entry is editable text. Text opens the embedded Grafita
    // editor; everything else falls back to the quick-look preview. The
    // classification runs on Grafita's worker, so a large file cannot stall
    // the folder while it is being read.
    function requestPreview() {
        const index = root.controller.indexForToken(root.controller.selectedToken)
        if (index < 0)
            return
        const path = root.controller.entryPath(index)
        // A folder has no bytes to classify, so asking would be a wasted read.
        if (path.length === 0 || root.controller.entryKind(index) === "directory") {
            root.owner.quickLookOpen = true
            return
        }
        grafitaEditorState.requestPreview(path)
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

    // How the reader reads, as Grafita stores it. Held here beside the
    // document state rather than inside either surface, so the peek and the
    // editor cannot drift apart, and re-read when a surface opens so a size
    // changed in Grafita — or in another folder view — is the one shown.
    GrafitaPreferences {
        id: readingPreferences
    }

    QuickLookView {
        id: quickLookDialog
        controller: root.controller
        owner: root.owner
        panel: root.panel
        player: mediaPlayerState
        reading: readingPreferences
    }

    // El reproductor incrustado detrás del modal de `Espacio`. Como el editor,
    // no sabe nada de carpetas: recibe una ruta y responde con una sesión o con
    // un rechazo, y no construye nada hasta que se lo piden.
    SideritaPlayer {
        id: mediaPlayerState
    }

    // The document state behind the embedded editor. It holds no folder
    // knowledge: it is handed a path and answers with an open document or a
    // refusal, exactly as the standalone application will drive it.
    GrafitaEditor {
        id: grafitaEditorState
        // Not text, or text this build cannot map back to its bytes: the
        // preview shows it read-only rather than pretending it is editable.
        onPreviewDeclined: function(path, reason) {
            root.owner.quickLookOpen = true
        }
    }

    GrafitaEditorDialog {
        id: grafitaEditorDialog
        editor: grafitaEditorState
        owner: root.owner
        backdrop: root.panel
        reading: readingPreferences
    }

    PhoneMediaDialog {
        id: phoneMediaDialog
        controller: root.controller
        owner: root.owner
        backdrop: root.panel
    }
}
