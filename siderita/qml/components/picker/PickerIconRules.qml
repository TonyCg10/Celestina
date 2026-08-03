import QtQuick
import org.celestina.siderita 1.0

// Same folder-type/tone rules the main folder view uses (FolderView.qml's
// `folderTypeIcons`/`entryIconTone`), factored out so PickerWindow.qml stays
// a coordinator: a folder shown in the picker looks like the one the user
// already knows from Siderita, instead of the generic glyph.
QtObject {
    id: root

    required property var pickerController

    property var folderTypeIcons: ({})

    function rebuild() {
        var defs = CelestinaFolderTypeIcons.defs
        var m = {}
        for (var k in defs) {
            var p = root.pickerController.placePath(k)
            if (p.length > 0)
                m[p] = defs[k]
        }
        root.folderTypeIcons = m
    }

    function folderIcon(path) {
        return (path && root.folderTypeIcons[path]) ? root.folderTypeIcons[path] : "folder"
    }

    function entryIconTone(kind) {
        return kind === "directory" ? CelestinaIcon.Folder
             : kind === "symlink" ? CelestinaIcon.Symlink
             : CelestinaIcon.File
    }
}
