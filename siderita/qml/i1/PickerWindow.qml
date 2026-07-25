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


    // Las mismas escalas persistidas que usa la ventana principal (el deslizador
    // de "Tamaño"): un diálogo que ignora la preferencia de tamaño del usuario
    // es un diálogo de otra aplicación. Se leen una vez, al abrir.
    property real iconScale: 1.0
    property real textScale: 1.0

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
            // Al margen superior hay que ir: una Flickable no recoloca su
            // contenido cuando el margen cambia o cuando llegan filas nuevas, y
            // la primera fila nacía debajo de la pastilla en vez de bajo ella.
            // Además una carpeta nueva se lee desde arriba.
            Qt.callLater(function() {
                entryGrid.contentY = entryGrid.originY - entryGrid.topMargin
            })
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
        iconScale = controller.savedContentIconScale()
        textScale = controller.savedContentTextScale()
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

    // El ancla del rango: dónde empezó la selección con la que Mayúsculas
    // cuenta. Un clic normal la mueve; Mayúsculas la respeta.
    property int anchorIndex: -1

    // Selecciona de `from` a `to` inclusive, saltando lo que este diálogo no
    // puede elegir (en modo carpeta, todo lo que no sea carpeta).
    function selectRange(from, to, additive) {
        if (from < 0 || to < 0)
            return
        var s = additive ? Object.assign({}, chosen) : {}
        const lo = Math.min(from, to)
        const hi = Math.max(from, to)
        for (var i = lo; i <= hi && i < entryGrid.count; i++) {
            if (picker.directory && controller.entryKind(i) !== "directory")
                continue
            s[controller.entryToken(i)] = true
        }
        chosen = s
        chosenCount = Object.keys(s).length
    }

    // Las celdas que toca un rectángulo, por aritmética y no por recorrido: la
    // rejilla es regular, así que se sabe qué filas y columnas cruza sin
    // preguntarle a cada una de las mil que puede haber.
    function selectIn(x1, y1, x2, y2, additive) {
        const cw = entryGrid.cellWidth
        const ch = entryGrid.cellHeight
        const cols = entryGrid.columns
        if (cw <= 0 || ch <= 0 || cols <= 0)
            return
        var s = additive ? Object.assign({}, chosen) : {}
        const c0 = Math.max(0, Math.floor(x1 / cw))
        const c1 = Math.min(cols - 1, Math.floor(x2 / cw))
        const r0 = Math.max(0, Math.floor(y1 / ch))
        const r1 = Math.floor(y2 / ch)
        for (var r = r0; r <= r1; r++) {
            for (var c = c0; c <= c1; c++) {
                const i = r * cols + c
                if (i < 0 || i >= entryGrid.count)
                    continue
                if (picker.directory && controller.entryKind(i) !== "directory")
                    continue
                s[controller.entryToken(i)] = true
            }
        }
        chosen = s
        chosenCount = Object.keys(s).length
    }

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
    // La misma anatomía que la ventana principal: lienzo negro, una caja de
    // contenido que no lo es, y los controles como pastillas que flotan encima
    // en vez de barras que le roban sitio al contenido. Un diálogo que no se
    // parece a la aplicación que lo sirve es un diálogo prestado.
    Item {
        anchors.fill: parent

        Rectangle {
            id: contentBox
            x: 12
            y: 12
            width: parent.width - 24
            height: parent.height - 24
            radius: CelestinaTheme.radiusLg
            color: CelestinaTheme.surface
            border.width: 1
            border.color: CelestinaTheme.border
            clip: true

            // La rejilla: la vista de un selector de archivos es de
            // reconocimiento, no de lectura — se busca *esa* foto, y una
            // miniatura la encuentra antes que su nombre.
            GridView {
                id: entryGrid
                anchors.fill: parent
                anchors.margins: 10
                clip: true
                model: entryModel
                currentIndex: -1
                cacheBuffer: 600
                boundsBehavior: Flickable.StopAtBounds
                focus: true
                // El sitio de las pastillas se reserva por dentro, así el
                // contenido pasa por detrás al desplazarse pero nunca se queda
                // escondido al principio ni al final. Con márgenes y no con
                // cabecera/pie: en una GridView el espaciador no desplaza la
                // primera fila y las celdas nacían debajo de la pastilla.
                topMargin: picker.saving ? 116 : 70
                bottomMargin: 72

                // Columnas que llenan el ancho: el hueco sobrante se reparte
                // entre ellas en vez de amontonarse a la derecha.
                readonly property int cellSide: Math.round(
                        116 * Math.max(picker.iconScale, picker.textScale))
                readonly property int columns: Math.max(1, Math.floor(width / cellSide))
                cellWidth: Math.floor(width / columns)
                // El alto lo dicta lo que hay dentro — icono, hueco, dos líneas
                // de nombre y su respiro — y no el ancho de la columna. Es la
                // misma cuenta que la cuadrícula principal; midiendo por el
                // ancho, el recuadro de selección arrastraba un vacío abajo.
                cellHeight: Math.round(72 * picker.iconScale) + 8
                            + Math.round(CelestinaTheme.fontCaption * 2.9 * picker.textScale)
                            + 20

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

                // Arrastrar sobre el hueco selecciona: el gesto de "coge estos
                // cuatro" no debería exigir una mano en el teclado. Vive bajo
                // los delegados (z: -1), así que una celda se lleva su clic y
                // el hueco se lleva el arrastre.
                MouseArea {
                    id: bandArea
                    z: -1
                    x: 0
                    y: 0
                    width: Math.max(entryGrid.contentWidth, entryGrid.width)
                    height: Math.max(entryGrid.contentHeight, entryGrid.height)

                    // Sin esto la Flickable se lleva el arrastre a los pocos
                    // píxeles y la banda se convierte en un desplazamiento.
                    // Sólo cuando hay algo que seleccionar en grupo: si el que
                    // pregunta quiere un archivo, arrastrar sigue desplazando.
                    preventStealing: picker.multiple

                    property real startX: 0
                    property real startY: 0
                    property bool banding: false
                    property bool additive: false

                    readonly property real bx: Math.min(startX, mouseX)
                    readonly property real by: Math.min(startY, mouseY)
                    readonly property real bw: Math.abs(mouseX - startX)
                    readonly property real bh: Math.abs(mouseY - startY)

                    onPressed: function(mouse) {
                        entryGrid.forceActiveFocus()
                        startX = mouse.x
                        startY = mouse.y
                        additive = picker.multiple
                                   && (mouse.modifiers & Qt.ControlModifier)
                        banding = false
                        if (!additive)
                            picker.clearChosen()
                    }
                    onPositionChanged: {
                        if (!picker.multiple)
                            return
                        // Un temblor no es un arrastre.
                        if (!banding && (bw > 4 || bh > 4))
                            banding = true
                        if (banding)
                            picker.selectIn(bx, by, bx + bw, by + bh, additive)
                    }
                    onReleased: banding = false
                    onCanceled: banding = false

                    Rectangle {
                        visible: bandArea.banding
                        x: bandArea.bx
                        y: bandArea.by
                        width: bandArea.bw
                        height: bandArea.bh
                        radius: CelestinaTheme.radiusXs
                        color: CelestinaTheme.surfaceSelected
                        border.width: 1
                        border.color: CelestinaTheme.borderStrong
                    }
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
                    // Sólo lo que el proveedor de miniaturas sabe servir; el
                    // resto se queda con su icono de tipo, que ya es
                    // informativo.
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
                        y: 10
                        width: Math.round(72 * picker.iconScale)
                        height: width
                        radius: CelestinaTheme.radiusSm
                        clip: true
                        opacity: cell.selectable ? 1 : 0.4
                        color: cell.isDirectory ? CelestinaTheme.glyphDirectory
                                                : CelestinaTheme.glyphFile

                        IconImage {
                            anchors.centerIn: parent
                            visible: !preview.ready
                            width: Math.round(54 * picker.iconScale)
                            height: width
                            sourceSize: Qt.size(width, width)
                            name: cell.isDirectory ? "folder" : "text-x-generic"
                            source: CelestinaTheme.fallbackIcon(
                                        cell.isDirectory ? "folder" : "file")
                        }

                        // La caché compartida de freedesktop, la misma que lee
                        // la ventana principal: si hay miniatura, aparece; si
                        // no, el icono se queda y nadie espera por nada.
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
                        font.pixelSize: Math.round(
                                CelestinaTheme.fontCaption * picker.textScale)
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
                            if (picker.multiple && (mouse.modifiers & Qt.ShiftModifier)
                                    && picker.anchorIndex >= 0) {
                                picker.selectRange(picker.anchorIndex, cell.index,
                                                   mouse.modifiers & Qt.ControlModifier)
                            } else if (picker.multiple
                                       && (mouse.modifiers & Qt.ControlModifier)) {
                                picker.toggleChosen(cell.token)
                                picker.anchorIndex = cell.index
                            } else {
                                picker.selectOnly(cell.token)
                                picker.anchorIndex = cell.index
                            }
                            if (picker.saving && !cell.isDirectory)
                                nameField.text = cell.name
                        }
                        onDoubleClicked: picker.activate(cell.index)
                    }
                }
            }
        }

        // ── Las pastillas de arriba ──────────────────────────────────────
        // Quién pregunta y dónde estamos, en la misma pastilla: son la misma
        // frase — "esta aplicación te está pidiendo algo de aquí".
        Item {
            id: topPills
            x: contentBox.x + 12
            y: contentBox.y + 12
            width: contentBox.width - 24
            height: picker.saving ? 100 : 54

            GlassPill {
                id: pathPill
                width: parent.width - navRow.width - 12
                height: 54
                radius: CelestinaTheme.radiusSm

                Text {
                    id: askedBy
                    x: 14
                    y: 9
                    width: parent.width - 28
                    text: picker.appId.length > 0
                          ? "Para " + picker.appId : "Solicitado por otra aplicación"
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontMini
                    elide: Text.ElideRight
                }

                Text {
                    x: 14
                    y: askedBy.y + askedBy.height + 2
                    width: parent.width - 28
                    text: controller.currentPath
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                    elide: Text.ElideMiddle
                }
            }

            Row {
                id: navRow
                anchors.right: parent.right
                y: 10
                spacing: 8

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

            // Guardar: el nombre va debajo, en su propia pastilla.
            TextField {
                id: nameField
                visible: picker.saving
                width: parent.width
                height: 38
                y: 62
                placeholderText: "Nombre del archivo"
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                leftPadding: 14
                rightPadding: 14
                onAccepted: if (picker.canAccept) picker.answer(picker.chosenPaths())
                background: GlassPill {
                    radius: CelestinaTheme.radiusSm
                    fill: CelestinaTheme.inputFill
                    border.width: 1
                    border.color: nameField.activeFocus ? CelestinaTheme.focus
                                                        : "transparent"
                }
            }
        }

        // ── Las pastillas de abajo ───────────────────────────────────────
        // Las dos únicas acciones que un diálogo ajeno puede ofrecer, y el
        // filtro de tipos que el que pregunta pidió.
        Item {
            id: bottomPills
            x: contentBox.x + 12
            width: contentBox.width - 24
            height: 38
            y: contentBox.y + contentBox.height - height - 12

            ComboBox {
                id: filterCombo
                visible: picker.filterRows.length > 1
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(300, parent.width * 0.4)
                height: 34
                model: picker.filterRows
                textRole: "label"
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontLabel
                onActivated: picker.applyFilter(currentIndex)

                contentItem: Text {
                    leftPadding: 12
                    rightPadding: filterCombo.indicator.width + 6
                    text: filterCombo.displayText
                    color: CelestinaTheme.text
                    font: filterCombo.font
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                }

                background: GlassPill {
                    fill: filterCombo.hovered ? CelestinaTheme.surfaceHover
                                              : CelestinaTheme.controlFill
                }
            }

            Text {
                anchors.left: filterCombo.visible ? filterCombo.right : parent.left
                anchors.leftMargin: filterCombo.visible ? 14 : 4
                anchors.verticalCenter: parent.verticalCenter
                text: picker.multiple && picker.chosenCount > 1
                      ? picker.chosenCount + " seleccionados" : ""
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
            }

            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 10

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

    // ── GlassPill ────────────────────────────────────────────────────────
    // Cristal debajo, tinte de estado encima: los tokens de relleno son
    // translúcidos, así que el tinte deja ver el desenfoque en vez de taparlo.
    // Captura mientras haya rejilla que desenfocar debajo.
    component GlassPill: Rectangle {
        id: glassPill

        property color fill: CelestinaTheme.controlFill

        radius: CelestinaTheme.radiusSm
        color: "transparent"

        GlassSurface {
            anchors.fill: parent
            backdropSource: entryGrid
            captureEnabled: entryGrid.contentHeight > entryGrid.height
            liveCapture: true
            cornerRadius: glassPill.radius
        }

        Rectangle {
            anchors.fill: parent
            radius: glassPill.radius
            color: glassPill.fill
            Behavior on color {
                ColorAnimation { duration: CelestinaTheme.motionFast }
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
            // Deshabilitado deja de ser primario: texto apagado sobre el acento
            // clarito no se leía. Un botón que no se puede pulsar tampoco tiene
            // por qué gritar.
            color: !control.enabled ? CelestinaTheme.textMuted
                   : control.primary ? CelestinaTheme.canvas : CelestinaTheme.text
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }

        // Cristal también aquí: los botones flotan sobre la rejilla como los
        // de la ventana principal, no sobre una banda que no existe.
        background: GlassPill {
            fill: !control.enabled ? CelestinaTheme.controlFill
                  : control.primary
                    ? (control.down ? Qt.darker(CelestinaTheme.accent, 1.18)
                       : control.hovered ? Qt.darker(CelestinaTheme.accent, 1.08)
                       : CelestinaTheme.accent)
                    : (control.down ? CelestinaTheme.surfaceStrong
                       : control.hovered ? CelestinaTheme.surfaceHover
                       : CelestinaTheme.controlFill)
            border.width: control.primary && control.enabled ? 0 : 1
            border.color: CelestinaTheme.border
        }
    }
}
