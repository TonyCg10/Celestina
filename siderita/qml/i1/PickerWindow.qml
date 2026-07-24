import QtQuick
import QtQuick.Window
import QtQuick.Controls
import QtQuick.Controls.impl
import org.celestina.siderita 1.0
import org.celestina.siderita.internal 1.0

// ─── PickerWindow ─────────────────────────────────────────────────────────────
// The dialog other applications get when they ask the desktop for a file. It is
// driven by `org.freedesktop.impl.portal.FileChooser` (src/portal.rs), so the
// asking application never knows it is talking to Siderita — it asked the
// portal, and the portal was told to route here.
//
// A window of its own, with its own controller: a picker is not a tab, it can
// outlive or precede the main window, and several can be open at once (one per
// requesting application). It deliberately reuses the browsing core rather than
// the browsing *chrome* — no tabs, no drag-and-drop, no write verbs. A dialog
// that can rename and delete while an application waits on it is a dialog that
// can surprise you.
// ──────────────────────────────────────────────────────────────────────────────
Window {
    id: picker

    // ── The request, as the portal described it ──────────────────────────
    required property string token
    required property string mode          // "open" | "save" | "saves"
    required property string appId
    required property string requestTitle
    required property string acceptLabel
    required property bool multiple
    required property bool directory
    required property string startFolder
    required property string suggestedName
    // Each entry is `name\tpattern|pattern` — the caller's type filters, with
    // its MIME types already translated to name patterns by the backend.
    required property var filters


    readonly property bool saving: mode === "save" || mode === "saves"
    readonly property string acceptText:
            acceptLabel.length > 0 ? acceptLabel
            : saving ? "Guardar"
            : directory ? "Elegir carpeta"
            : "Abrir"

    width: 1180
    height: 800
    minimumWidth: 640
    minimumHeight: 460
    visible: true
    color: CelestinaTheme.canvas
    title: requestTitle.length > 0 ? requestTitle
           : saving ? "Guardar archivo" : "Abrir archivo"
    // A picker belongs to the application that asked for it, so it is a dialog,
    // not another file manager window in the switcher.
    flags: Qt.Dialog

    // ── Browsing ─────────────────────────────────────────────────────────
    // Its own controller: the picker browses independently of whatever the main
    // window is showing, and works when there is no main window at all.
    SideritaController {
        id: controller
    }

    SideritaEntryModel {
        id: entryModel
    }

    Connections {
        target: controller
        function onRowsReady(names, tokens, kinds, subtitles, paths, sections, sizes, dates) {
            entryModel.setRows(names, tokens, kinds, subtitles, paths, sections, sizes, dates)
        }
    }

    // The filters as the combo shows them, with an always-present "everything"
    // row: a chooser must never be able to hide a file with no way back.
    readonly property var filterRows: {
        var rows = [{ label: "Todos los archivos", patterns: [] }]
        for (var i = 0; i < filters.length; i++) {
            var cut = filters[i].indexOf("\t")
            if (cut <= 0)
                continue
            rows.push({
                label: filters[i].substring(0, cut),
                patterns: filters[i].substring(cut + 1).split("|").filter(function(p) {
                    return p.length > 0
                })
            })
        }
        return rows
    }

    function applyFilter(index) {
        if (index < 0 || index >= filterRows.length)
            return
        controller.applyNameFilters(filterRows[index].patterns)
        clearChosen()
    }

    Component.onCompleted: {
        if (startFolder.length > 0)
            controller.startAt(startFolder)
        else
            controller.start()
        nameField.text = suggestedName
        // The caller's first filter is the one it expects to be active.
        if (filterRows.length > 1) {
            filterCombo.currentIndex = 1
            applyFilter(1)
        }
        entryGrid.forceActiveFocus()
    }

    // What Accept will hand back. A directory request answers with the folder
    // being shown unless one is selected; a save answers with the typed name in
    // the current folder; an open answers with the selection.
    function chosenPaths() {
        if (saving) {
            const name = nameField.text.trim()
            if (name.length === 0 || name.indexOf("/") >= 0)
                return []
            return [controller.currentPath + "/" + name]
        }
        if (directory) {
            const selected = selectedPaths(true)
            return selected.length > 0 ? selected : [controller.currentPath]
        }
        return selectedPaths(false)
    }

    function selectedPaths(foldersOnly) {
        const out = []
        for (var i = 0; i < entryGrid.count; i++) {
            if (chosen[controller.entryToken(i)] !== true)
                continue
            const kind = controller.entryKind(i)
            if (foldersOnly && kind !== "directory")
                continue
            if (!foldersOnly && !directory && kind === "directory")
                continue
            out.push(controller.entryPath(i))
        }
        return out
    }

    readonly property bool canAccept: chosenPaths().length > 0

    // Token-keyed, like the main view's selection, so it survives a re-sort.
    property var chosen: ({})
    property int chosenCount: 0

    function clearChosen() {
        chosen = ({})
        chosenCount = 0
    }
    function selectOnly(token) {
        var s = {}
        s[token] = true
        chosen = s
        chosenCount = 1
    }
    function toggleChosen(token) {
        var s = Object.assign({}, chosen)
        if (s[token]) delete s[token]; else s[token] = true
        chosen = s
        chosenCount = Object.keys(s).length
    }
    function isChosen(token) { return chosen[token] === true }

    function activate(index) {
        const kind = controller.entryKind(index)
        const path = controller.entryPath(index)
        if (kind === "directory") {
            clearChosen()
            controller.openLocation(path)
        } else if (!directory && !saving) {
            picker.answer([path])
        } else if (saving) {
            nameField.text = controller.entryNames[index]
        }
    }

    // Answering does not close the window: the backend does, once the reply is
    // actually on its way back to the asking application (onPickAnswered). The
    // window only stops accepting input meanwhile.
    property bool answered: false

    function answer(paths) {
        if (answered)
            return
        answered = true
        portalService.answer(picker.token, paths)
    }
    function cancel() {
        // An empty answer is the cancel: the backend tells the difference.
        answer([])
    }

    onClosing: function(close) {
        close.accepted = false
        picker.cancel()
    }

    Connections {
        target: controller
        // A folder change drops a selection that no longer means anything.
        function onCurrentPathChanged() { picker.clearChosen() }
    }

    // Los botones laterales del ratón navegan, igual que en la ventana
    // principal: un diálogo de archivos se recorre con la mano en el ratón, y
    // el gesto ya existe en el resto de la aplicación.
    TapHandler {
        acceptedButtons: Qt.BackButton | Qt.ForwardButton
        gesturePolicy: TapHandler.ReleaseWithinBounds
        onTapped: function(eventPoint, button) {
            if (controller.loading)
                return
            if (button === Qt.BackButton && controller.canGoBack)
                controller.goBack()
            else if (button === Qt.ForwardButton && controller.canGoForward)
                controller.goForward()
        }
    }

    // ── Chrome ───────────────────────────────────────────────────────────
    Rectangle {
        anchors.fill: parent
        color: CelestinaTheme.canvas

        // Header: where we are, and (for a save) what it will be called.
        Rectangle {
            id: header
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: picker.saving ? 104 : 60
            color: CelestinaTheme.canvasRaised

            Text {
                id: askedBy
                x: 16
                y: 10
                width: parent.width - 32
                text: picker.appId.length > 0
                      ? "Para " + picker.appId : "Solicitado por otra aplicación"
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
                elide: Text.ElideRight
            }

            Text {
                x: 16
                y: askedBy.y + askedBy.height + 2
                width: parent.width - 220
                text: controller.currentPath
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                elide: Text.ElideMiddle
            }

            Row {
                anchors.right: parent.right
                anchors.rightMargin: 14
                y: askedBy.y + askedBy.height - 2
                spacing: 6

                PickerButton {
                    text: "↑"
                    help: "Subir"
                    enabled: controller.canGoUp && !controller.loading
                    onClicked: controller.goUp()
                }
                PickerButton {
                    text: "⌂"
                    help: "Inicio"
                    onClicked: controller.goHome()
                }
            }

            TextField {
                id: nameField
                visible: picker.saving
                x: 16
                width: parent.width - 32
                height: 32
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 8
                placeholderText: "Nombre del archivo"
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                leftPadding: 10
                rightPadding: 10
                onAccepted: if (picker.canAccept) picker.answer(picker.chosenPaths())
                background: Rectangle {
                    radius: CelestinaTheme.radiusSm
                    color: CelestinaTheme.inputFill
                    border.width: 1
                    border.color: nameField.activeFocus ? CelestinaTheme.focus
                                                        : CelestinaTheme.inputBorder
                }
            }
        }

        // La rejilla: la vista de un selector de archivos es de reconocimiento,
        // no de lectura — se busca *esa* foto, y una miniatura la encuentra
        // antes que su nombre. Celdas grandes por la misma razón.
        GridView {
            id: entryGrid
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: header.bottom
            anchors.bottom: footer.top
            anchors.margins: 10
            clip: true
            model: entryModel
            currentIndex: -1
            cacheBuffer: 600
            boundsBehavior: Flickable.StopAtBounds
            focus: true
            // Columnas que llenan el ancho: el hueco sobrante se reparte entre
            // ellas en vez de amontonarse a la derecha.
            readonly property int cellSide: 136
            readonly property int columns: Math.max(1, Math.floor(width / cellSide))
            cellWidth: Math.floor(width / columns)
            cellHeight: cellSide + 26
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    picker.cancel()
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    if (currentIndex >= 0)
                        picker.activate(currentIndex)
                } else if (event.key === Qt.Key_Backspace) {
                    controller.goUp()
                } else {
                    return
                }
                event.accepted = true
            }

            delegate: Item {
                id: cell

                required property int index
                required property string name
                required property string token
                required property string kind
                required property string path

                readonly property bool isDirectory: kind === "directory"
                readonly property bool selectable: picker.directory ? isDirectory : true
                readonly property bool chosen: picker.isChosen(token)
                // Sólo lo que el proveedor de miniaturas sabe servir; el resto
                // se queda con su icono de tipo, que ya es informativo.
                readonly property bool previewable: !isDirectory
                        && /\.(png|jpe?g|gif|webp|bmp|svg|avif|tiff?|ico|heic)$/i.test(name)

                width: entryGrid.cellWidth
                height: entryGrid.cellHeight

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: 4
                    radius: CelestinaTheme.radiusSm
                    color: cell.chosen ? CelestinaTheme.surfaceSelected
                           : cellMouse.containsMouse ? CelestinaTheme.surfaceHover
                           : "transparent"
                    border.width: cell.chosen ? 1 : 0
                    border.color: CelestinaTheme.borderStrong
                    Behavior on color {
                        ColorAnimation { duration: CelestinaTheme.motionFast }
                    }
                }

                Rectangle {
                    id: tile
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: 12
                    width: 84
                    height: 84
                    radius: CelestinaTheme.radiusSm
                    clip: true
                    opacity: cell.selectable ? 1 : 0.4
                    color: cell.isDirectory ? CelestinaTheme.glyphDirectory
                                            : CelestinaTheme.glyphFile

                    IconImage {
                        anchors.centerIn: parent
                        visible: !preview.ready
                        width: 46
                        height: 46
                        name: cell.isDirectory ? "folder" : "text-x-generic"
                        source: CelestinaTheme.fallbackIcon(
                                    cell.isDirectory ? "folder" : "file")
                    }

                    // La caché compartida de freedesktop, la misma que lee la
                    // ventana principal: si hay miniatura, aparece; si no, el
                    // icono se queda y nadie espera por nada.
                    Image {
                        id: preview
                        anchors.fill: parent
                        anchors.margins: 1
                        readonly property bool ready: cell.previewable
                                                      && status === Image.Ready
                        visible: opacity > 0
                        opacity: ready ? 1 : 0
                        source: cell.previewable
                                ? "image://thumb/" + encodeURIComponent(cell.path) : ""
                        sourceSize.width: 256
                        sourceSize.height: 256
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        cache: true
                        smooth: true
                        Behavior on opacity {
                            NumberAnimation { duration: CelestinaTheme.motionNormal }
                        }
                    }
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: tile.y + tile.height + 8
                    width: parent.width - 14
                    horizontalAlignment: Text.AlignHCenter
                    text: cell.name
                    color: cell.selectable ? CelestinaTheme.text
                                           : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    elide: Text.ElideRight
                    maximumLineCount: 2
                    wrapMode: Text.Wrap
                }

                MouseArea {
                    id: cellMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: function(mouse) {
                        entryGrid.currentIndex = cell.index
                        entryGrid.forceActiveFocus()
                        if (!cell.selectable)
                            return
                        if (picker.multiple && (mouse.modifiers & Qt.ControlModifier))
                            picker.toggleChosen(cell.token)
                        else
                            picker.selectOnly(cell.token)
                        if (picker.saving && !cell.isDirectory)
                            nameField.text = cell.name
                    }
                    onDoubleClicked: picker.activate(cell.index)
                }
            }
        }

        // Footer: the two buttons an application's dialog is allowed to have.
        Rectangle {
            id: footer
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 72
            color: CelestinaTheme.canvasRaised

            Text {
                x: 16
                anchors.verticalCenter: parent.verticalCenter
                text: picker.multiple && picker.chosenCount > 1
                      ? picker.chosenCount + " seleccionados" : ""
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
            }

            ComboBox {
                id: filterCombo
                visible: picker.filterRows.length > 1
                anchors.left: parent.left
                anchors.leftMargin: 22
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(300, parent.width * 0.4)
                height: 34
                model: picker.filterRows
                textRole: "label"
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontLabel
                onActivated: picker.applyFilter(currentIndex)

                contentItem: Text {
                    leftPadding: 10
                    rightPadding: filterCombo.indicator.width + 6
                    text: filterCombo.displayText
                    color: CelestinaTheme.text
                    font: filterCombo.font
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                }

                background: Rectangle {
                    radius: CelestinaTheme.radiusSm
                    color: filterCombo.hovered ? CelestinaTheme.surfaceHover
                                               : CelestinaTheme.controlFill
                    border.width: 1
                    border.color: CelestinaTheme.border
                }
            }

            Row {
                anchors.right: parent.right
                anchors.rightMargin: 22
                anchors.verticalCenter: parent.verticalCenter
                spacing: 12

                PickerButton {
                    text: "Cancelar"
                    onClicked: picker.cancel()
                }
                PickerButton {
                    text: picker.acceptText
                    primary: true
                    enabled: picker.canAccept
                    onClicked: picker.answer(picker.chosenPaths())
                }
            }
        }
    }




    // A button local to the picker: the suite's PillButton lives inside
    // MainI1's root object and inline components cannot be reached across files.
    component PickerButton: Button {
        id: control

        property bool primary: false
        property string help: ""

        hoverEnabled: true
        implicitHeight: 34
        leftPadding: 18
        rightPadding: 18
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontLabel
        ToolTip.visible: help.length > 0 && hovered
        ToolTip.text: help

        contentItem: Text {
            text: control.text
            font: control.font
            color: !control.enabled ? CelestinaTheme.textMuted
                   : control.primary ? CelestinaTheme.canvas : CelestinaTheme.text
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }

        background: Rectangle {
            radius: CelestinaTheme.radiusSm
            opacity: control.enabled ? 1 : 0.5
            color: control.primary
                   ? (control.down ? Qt.darker(CelestinaTheme.accent, 1.18)
                      : control.hovered ? Qt.darker(CelestinaTheme.accent, 1.08)
                      : CelestinaTheme.accent)
                   : (control.down ? CelestinaTheme.surfaceStrong
                      : control.hovered ? CelestinaTheme.surfaceHover
                      : CelestinaTheme.controlFill)
            border.width: control.primary ? 0 : 1
            border.color: CelestinaTheme.border
            Behavior on color {
                ColorAnimation { duration: CelestinaTheme.motionFast }
            }
        }
    }
}
