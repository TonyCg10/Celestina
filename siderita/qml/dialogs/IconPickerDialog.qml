import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.siderita 1.0

    // ── Icon picker (Cambiar icono…) ─────────────────────────────────
    // Pick a custom icon for one entry from the theme's folder-type (or
    // file-type) variants; the choice persists per path via the controller.
CelestinaModalLayer {
    id: iconPicker
    property var controller
    property var owner
    property var panel   // mainPanel: ayudas de selección y medios
    anchors.fill: parent
    z: 69
    onDismissRequested: iconPicker.dismiss()

    property string targetPath: ""
    property bool forFolder: true
    readonly property var folderOptions: [
        "folder", "folder-documents", "folder-download", "folder-music",
        "folder-pictures", "folder-videos", "folder-desktop", "folder-templates",
        "folder-publicshare", "folder-development", "folder-games", "folder-git",
        "folder-github", "folder-image", "folder-important", "folder-favorites",
        "folder-cloud", "folder-mail", "folder-print", "folder-script",
        "folder-sync", "folder-text", "folder-video"
    ]
    readonly property var fileOptions: [
        "text-x-generic", "text-x-script", "text-x-python", "text-html",
        "application-json", "image-x-generic", "video-x-generic",
        "audio-x-generic", "application-pdf", "application-x-archive",
        "application-x-executable", "font-x-generic", "application-x-desktop"
    ]
    readonly property var options: forFolder ? folderOptions : fileOptions

    function openFor(path, isFolder) {
        targetPath = path
        forFolder = isFolder
        shown = true
    }
    function choose(name) {
        // This tab updates now; the others re-read the file when they
        // are next activated (like bookmarks).
        controller.setCustomIcon(targetPath, name)
        shown = false
    }
    function dismiss() { shown = false }

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(520, owner.width - 48)
        height: Math.min(440, owner.height - 64)
        backdropSource: iconPicker.panel
        Accessible.role: Accessible.Dialog
        Accessible.name: "Cambiar icono"

        MouseArea { anchors.fill: parent }

        Text {
            id: iconPickerHeading
            x: 18
            y: 16
            text: "Elegir icono"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
        }

        GridView {
            id: iconGrid
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: iconPickerHeading.bottom
            anchors.bottom: iconPickerButtons.top
            anchors.topMargin: 12
            anchors.bottomMargin: 12
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            clip: true
            cellWidth: 78
            cellHeight: 78
            model: iconPicker.options

            delegate: Item {
                id: iconOpt
                required property string modelData
                width: iconGrid.cellWidth
                height: iconGrid.cellHeight

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: 4
                    radius: CelestinaTheme.radiusSm
                    color: iconOptMouse.containsMouse
                           ? CelestinaTheme.surfaceHover : CelestinaTheme.clear
                    border.width: panel.icons.customIcon(iconPicker.targetPath)
                                  === iconOpt.modelData
                                  ? CelestinaTheme.borderHairline : 0
                    border.color: CelestinaTheme.dividerStrong

                    CelestinaIcon {
                        anchors.centerIn: parent
                        width: 42
                        height: 42
                        // Sin esto, un icono simbólico (los que el tema
                        // sólo trae a 16 px) se dibuja a su tamaño real
                        // y queda diminuto junto a una carpeta: pedir el
                        // tamaño obliga a rasterizar el SVG a esa medida.
                        sourceSize: Qt.size(42, 42)
                        name: iconOpt.modelData
                        fallbackName: iconPicker.forFolder ? "folder" : "file"
                        tone: iconPicker.forFolder
                              ? CelestinaIcon.Folder : CelestinaIcon.File
                        tintOverride: panel.icons.iconTint(iconPicker.targetPath)
                    }
                }

                MouseArea {
                    id: iconOptMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: iconPicker.choose(iconOpt.modelData)
                }
            }
        }

        Row {
            id: iconPickerButtons
            anchors.right: parent.right
            anchors.rightMargin: 18
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 16
            spacing: 8

            CelestinaButton {
                text: "Restablecer"
                onClicked: iconPicker.choose("")
            }
            CelestinaButton {
                text: "Cerrar"
                role: CelestinaButton.Primary
                onClicked: iconPicker.dismiss()
            }
        }
    }
}
