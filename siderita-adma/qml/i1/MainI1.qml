import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import org.celestina.siderita 1.0

ApplicationWindow {
    id: window

    width: 1120
    height: 720
    minimumWidth: 680
    minimumHeight: 480
    visible: true
    color: CelestinaTheme.canvas
    title: "Siderita · Iteración 1"

    component NavButton: ToolButton {
        id: control

        required property string iconName
        required property string fallbackIcon
        required property string helpText

        implicitWidth: CelestinaTheme.controlHeight
        implicitHeight: CelestinaTheme.controlHeight
        hoverEnabled: true
        ToolTip.visible: hovered
        ToolTip.text: helpText
        ToolTip.delay: 550
        Accessible.name: helpText
        display: AbstractButton.IconOnly
        icon.name: iconName
        icon.source: CelestinaTheme.fallbackIcon(fallbackIcon)
        icon.width: CelestinaTheme.iconSm
        icon.height: CelestinaTheme.iconSm
        icon.color: control.enabled
                    ? CelestinaTheme.text
                    : CelestinaTheme.textMuted

        background: Rectangle {
            radius: CelestinaTheme.radiusSm
            color: control.hovered
                   ? CelestinaTheme.surfaceHover
                   : CelestinaTheme.surface
            border.width: control.activeFocus ? 1 : 0
            border.color: CelestinaTheme.focus

            Behavior on color {
                ColorAnimation {
                    duration: CelestinaTheme.motionFast
                }
            }
        }
    }

    SideritaController {
        id: controller
    }

    Shortcut {
        sequence: "Alt+Left"
        enabled: controller.canGoBack && !controller.loading
        onActivated: controller.goBack()
    }

    Shortcut {
        sequence: "Alt+Right"
        enabled: controller.canGoForward && !controller.loading
        onActivated: controller.goForward()
    }

    Shortcut {
        sequence: "Alt+Up"
        enabled: controller.canGoUp && !controller.loading
        onActivated: controller.goUp()
    }

    Shortcut {
        sequence: "Ctrl+L"
        onActivated: pathPill.beginEditing()
    }

    Shortcut {
        sequence: "Ctrl+F"
        onActivated: {
            searchField.forceActiveFocus()
            searchField.selectAll()
        }
    }

    Shortcut {
        sequence: "Ctrl+H"
        enabled: !controller.loading
        onActivated: controller.toggleHidden()
    }

    Shortcut {
        sequence: "F5"
        enabled: !controller.loading
        onActivated: controller.refresh()
    }

    Item {
        id: contentLayer
        anchors.fill: parent

        TapHandler {
            id: historyMouseButtons

            acceptedButtons: Qt.BackButton | Qt.ForwardButton
            gesturePolicy: TapHandler.ReleaseWithinBounds

            onTapped: function(eventPoint, button) {
                if (controller.loading)
                    return

                if (button === Qt.BackButton && controller.canGoBack)
                    controller.goBack()
                else if (button === Qt.ForwardButton
                         && controller.canGoForward)
                    controller.goForward()
            }
        }

        Rectangle {
            anchors.fill: parent
            color: CelestinaTheme.canvas

            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: CelestinaTheme.gradientStart }
                GradientStop { position: 0.55; color: CelestinaTheme.gradientMid }
                GradientStop { position: 1; color: CelestinaTheme.gradientEnd }
            }
        }

        Rectangle {
            id: topBar
            x: 20
            y: 18
            width: parent.width - 40
            height: 64
            radius: CelestinaTheme.radiusLg
            color: CelestinaTheme.surface
            border.width: 1
            border.color: CelestinaTheme.border

            NavButton {
                id: backButton
                x: 12
                anchors.verticalCenter: parent.verticalCenter
                iconName: "go-previous"
                fallbackIcon: "go-previous"
                helpText: "Atrás"
                enabled: controller.canGoBack && !controller.loading
                onClicked: controller.goBack()
            }

            NavButton {
                id: forwardButton
                x: backButton.x + backButton.width + 6
                anchors.verticalCenter: parent.verticalCenter
                iconName: "go-next"
                fallbackIcon: "go-next"
                helpText: "Adelante"
                enabled: controller.canGoForward && !controller.loading
                onClicked: controller.goForward()
            }

            NavButton {
                id: upButton
                x: forwardButton.x + forwardButton.width + 6
                anchors.verticalCenter: parent.verticalCenter
                iconName: "go-up"
                fallbackIcon: "go-up"
                helpText: "Subir"
                enabled: controller.canGoUp && !controller.loading
                onClicked: controller.goUp()
            }

            Rectangle {
                id: pathPill

                property bool editing: false

                function beginEditing() {
                    editing = true
                    locationField.text = controller.currentPath
                    locationField.forceActiveFocus()
                    locationField.selectAll()
                }

                function cancelEditing() {
                    editing = false
                    fileList.forceActiveFocus()
                }

                function pathSegments(p) {
                    if (!p || p.length === 0)
                        return []
                    const parts = p.split("/")
                    const segs = []
                    let acc = ""
                    for (let idx = 0; idx < parts.length; idx++) {
                        const part = parts[idx]
                        if (part.length === 0) {
                            if (idx === 0)
                                segs.push({ name: "/", path: "/" })
                            continue
                        }
                        acc = acc + "/" + part
                        segs.push({ name: part, path: acc })
                    }
                    return segs
                }

                x: upButton.x + upButton.width + 10
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(180, searchField.x - x - 12)
                height: CelestinaTheme.controlHeight
                radius: CelestinaTheme.radiusSm
                clip: true
                color: CelestinaTheme.inputFill
                border.width: 1
                border.color: CelestinaTheme.inputBorder

                MouseArea {
                    anchors.fill: parent
                    visible: !pathPill.editing
                    cursorShape: Qt.IBeamCursor
                    Accessible.name: "Editar ubicación"
                    onClicked: pathPill.beginEditing()
                }

                Row {
                    id: crumbRow
                    anchors.right: parent.right
                    anchors.rightMargin: 13
                    anchors.verticalCenter: parent.verticalCenter
                    visible: !pathPill.editing
                    spacing: 3

                    Repeater {
                        id: crumbRepeater
                        model: pathPill.pathSegments(controller.currentPath)

                        delegate: Row {
                            id: crumb

                            required property var modelData
                            required property int index

                            spacing: 3
                            anchors.verticalCenter: parent.verticalCenter

                            Text {
                                visible: crumb.index > 0
                                anchors.verticalCenter: parent.verticalCenter
                                text: "›"
                                color: CelestinaTheme.textMuted
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontLabel
                            }

                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: crumbText.implicitWidth + 12
                                height: 24
                                radius: CelestinaTheme.radiusXs
                                color: crumbMouse.containsMouse
                                       ? CelestinaTheme.surfaceHover
                                       : "transparent"

                                Text {
                                    id: crumbText
                                    anchors.centerIn: parent
                                    text: crumb.modelData.name
                                    color: crumb.index === crumbRepeater.count - 1
                                           ? CelestinaTheme.text
                                           : CelestinaTheme.textMuted
                                    font.family: CelestinaTheme.sansFamily
                                    font.pixelSize: CelestinaTheme.fontLabel
                                }

                                MouseArea {
                                    id: crumbMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: controller.openLocation(
                                                   crumb.modelData.path)
                                }
                            }
                        }
                    }
                }

                TextField {
                    id: locationField

                    anchors.fill: parent
                    visible: pathPill.editing
                    leftPadding: 13
                    rightPadding: 13
                    color: CelestinaTheme.text
                    selectionColor: CelestinaTheme.accentStrong
                    selectedTextColor: CelestinaTheme.text
                    font.family: CelestinaTheme.monoFamily
                    font.pixelSize: CelestinaTheme.fontLabel
                    background: null
                    Accessible.name: "Ubicación"

                    onActiveFocusChanged: {
                        if (!activeFocus && pathPill.editing)
                            pathPill.editing = false
                    }

                    onAccepted: {
                        const location = text
                        pathPill.editing = false
                        controller.openLocation(location)
                        fileList.forceActiveFocus()
                    }

                    Keys.onPressed: function(event) {
                        if (event.key === Qt.Key_Escape) {
                            pathPill.cancelEditing()
                            event.accepted = true
                        }
                    }
                }

                Connections {
                    target: controller

                    function onCurrentPathChanged() {
                        if (pathPill.editing)
                            pathPill.editing = false
                    }
                }
            }

            TextField {
                id: searchField
                width: Math.min(260, Math.max(170, topBar.width * 0.24))
                height: CelestinaTheme.controlHeight
                x: refreshButton.x - width - 10
                anchors.verticalCenter: parent.verticalCenter
                placeholderText: "Buscar en esta carpeta"
                color: CelestinaTheme.text
                placeholderTextColor: CelestinaTheme.textMuted
                selectionColor: CelestinaTheme.accentStrong
                selectedTextColor: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                leftPadding: 13
                rightPadding: 13
                onTextEdited: searchDebounce.restart()

                background: Rectangle {
                    radius: CelestinaTheme.radiusSm
                    color: searchField.activeFocus
                           ? CelestinaTheme.inputFillFocus
                           : CelestinaTheme.inputFill
                    border.width: 1
                    border.color: searchField.activeFocus
                                  ? CelestinaTheme.focus
                                  : CelestinaTheme.inputBorder
                }
            }

            Timer {
                id: searchDebounce
                interval: 120
                repeat: false
                onTriggered: controller.applyQuery(searchField.text)
            }

            NavButton {
                id: refreshButton
                x: parent.width - width - 12
                anchors.verticalCenter: parent.verticalCenter
                iconName: "view-refresh"
                fallbackIcon: "view-refresh"
                helpText: "Actualizar"
                enabled: !controller.loading
                onClicked: controller.refresh()
            }
        }

        Rectangle {
            id: sidebar
            x: 20
            y: 98
            width: 184
            height: parent.height - y - 20
            radius: CelestinaTheme.radiusLg
            visible: parent.width >= 820
            color: CelestinaTheme.surface
            border.width: 1
            border.color: CelestinaTheme.border

            Text {
                x: 16
                y: 16
                text: "SIDERITA"
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                font.letterSpacing: 1.7
                font.weight: CelestinaTheme.weightDemiBold
            }

            Button {
                id: homeButton
                x: 10
                y: 46
                width: parent.width - 20
                height: CelestinaTheme.controlHeightLg
                text: "Inicio"
                icon.name: "go-home"
                icon.source: CelestinaTheme.fallbackIcon("go-home")
                icon.width: CelestinaTheme.iconSm
                icon.height: CelestinaTheme.iconSm
                icon.color: CelestinaTheme.text
                onClicked: controller.goHome()

                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                font.weight: CelestinaTheme.weightMedium

                background: Rectangle {
                    radius: CelestinaTheme.radiusSm
                    color: homeButton.hovered
                           ? CelestinaTheme.surfaceHover
                           : CelestinaTheme.controlFill
                }
            }

            Rectangle {
                x: 14
                y: parent.height - 78
                width: parent.width - 28
                height: 52
                radius: CelestinaTheme.radiusSm
                color: CelestinaTheme.badgeFill
                border.width: 1
                border.color: CelestinaTheme.border

                Text {
                    anchors.centerIn: parent
                    text: "SOLO LECTURA · I1"
                    color: CelestinaTheme.accent
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontMini
                    font.weight: CelestinaTheme.weightDemiBold
                    font.letterSpacing: 0.8
                }
            }
        }

        Rectangle {
            id: mainPanel

            property string viewMode: "list"   // "list" | "grid"

            x: sidebar.visible ? sidebar.x + sidebar.width + 14 : 20
            y: 98
            width: parent.width - x - 20
            height: parent.height - y - 20
            radius: CelestinaTheme.radiusLg
            color: CelestinaTheme.surface
            border.width: 1
            border.color: CelestinaTheme.border

            Text {
                id: panelTitle
                x: 18
                y: 15
                text: "Contenido"
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontTitle
                font.weight: CelestinaTheme.weightDemiBold
            }

            Rectangle {
                id: hiddenToggle
                x: panelTitle.x + panelTitle.width + 12
                anchors.verticalCenter: panelTitle.verticalCenter
                width: hiddenLabel.width + 20
                height: 26
                radius: CelestinaTheme.radiusXs
                color: controller.showHidden
                       ? CelestinaTheme.badgeAccentFill
                       : hiddenMouse.containsMouse
                         ? CelestinaTheme.surfaceHover
                         : CelestinaTheme.controlFill

                Behavior on color {
                    ColorAnimation { duration: CelestinaTheme.motionFast }
                }

                Accessible.role: Accessible.Button
                Accessible.name: "Mostrar u ocultar elementos ocultos"

                Text {
                    id: hiddenLabel
                    anchors.centerIn: parent
                    text: "Ocultos"
                    color: controller.showHidden
                           ? CelestinaTheme.accent
                           : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontMini
                    font.weight: CelestinaTheme.weightMedium
                }

                MouseArea {
                    id: hiddenMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: controller.toggleHidden()
                }
            }

            BusyIndicator {
                id: busy
                width: 28
                height: 28
                x: parent.width - width - 18
                y: 13
                running: controller.loading
                visible: running
            }

            NavButton {
                id: sortDirectionButton
                x: busy.x - width - 8
                y: 9
                iconName: controller.sortAscending
                          ? "view-sort-ascending"
                          : "view-sort-descending"
                fallbackIcon: controller.sortAscending
                              ? "view-sort-ascending"
                              : "view-sort-descending"
                helpText: controller.sortAscending
                          ? "Orden ascendente"
                          : "Orden descendente"
                onClicked: controller.toggleSortDirection()
            }

            Button {
                id: sortButton

                readonly property var labels: [
                    "Nombre", "Tamaño", "Fecha", "Tipo"
                ]

                x: sortDirectionButton.x - width - 8
                y: 11
                width: 116
                height: 34
                text: labels[controller.sortField]
                Accessible.name: "Ordenar por " + text
                onClicked: {
                    const point = sortButton.mapToItem(
                                    window.contentItem, 0, height + 6)
                    sortMenu.popup(window.contentItem, point)
                }

                contentItem: Text {
                    text: "Orden: " + sortButton.text
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    font.weight: CelestinaTheme.weightMedium
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                }

                background: Rectangle {
                    radius: CelestinaTheme.radiusSm
                    color: sortButton.hovered
                           ? CelestinaTheme.surfaceHover
                           : CelestinaTheme.controlFill
                    border.width: sortButton.activeFocus ? 1 : 0
                    border.color: CelestinaTheme.focus

                    Behavior on color {
                        ColorAnimation {
                            duration: CelestinaTheme.motionFast
                        }
                    }
                }
            }

            Button {
                id: viewToggle

                readonly property bool grid: mainPanel.viewMode === "grid"

                x: sortButton.x - width - 8
                y: 11
                width: 116
                height: 34
                text: grid ? "Lista" : "Cuadrícula"
                Accessible.name: "Cambiar vista"
                onClicked: mainPanel.viewMode = grid ? "list" : "grid"

                contentItem: Text {
                    text: viewToggle.text
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    font.weight: CelestinaTheme.weightMedium
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                }

                background: Rectangle {
                    radius: CelestinaTheme.radiusSm
                    color: viewToggle.hovered
                           ? CelestinaTheme.surfaceHover
                           : CelestinaTheme.controlFill
                    border.width: viewToggle.activeFocus ? 1 : 0
                    border.color: CelestinaTheme.focus

                    Behavior on color {
                        ColorAnimation {
                            duration: CelestinaTheme.motionFast
                        }
                    }
                }
            }

            Rectangle {
                x: 16
                y: 52
                width: parent.width - 32
                height: 1
                color: CelestinaTheme.border
            }

            ListView {
                id: fileList
                x: 8
                y: 60
                width: parent.width - 16
                height: parent.height - 100
                visible: mainPanel.viewMode === "list"
                model: controller.entryNames
                clip: true
                spacing: 2
                reuseItems: true
                cacheBuffer: 420
                boundsBehavior: Flickable.StopAtBounds
                activeFocusOnTab: true
                keyNavigationEnabled: false
                currentIndex: -1

                Connections {
                    target: controller

                    function onViewRevisionChanged() {
                        fileList.currentIndex = controller.indexForToken(
                                    controller.selectedToken)
                    }
                }

                function selectRow(i) {
                    if (i < 0 || i >= count)
                        return
                    currentIndex = i
                    controller.selectToken(controller.entryToken(i))
                    positionViewAtIndex(i, ListView.Contain)
                }

                function pageStep() {
                    return Math.max(
                        1, Math.floor(height / (CelestinaTheme.rowHeight + spacing)))
                }

                Keys.onPressed: function(event) {
                    if (count === 0)
                        return

                    const i = currentIndex

                    if (event.key === Qt.Key_Down) {
                        selectRow(Math.min(count - 1, i + 1))
                        event.accepted = true
                    } else if (event.key === Qt.Key_Up) {
                        selectRow(i < 0 ? count - 1 : Math.max(0, i - 1))
                        event.accepted = true
                    } else if (event.key === Qt.Key_Home) {
                        selectRow(0)
                        event.accepted = true
                    } else if (event.key === Qt.Key_End) {
                        selectRow(count - 1)
                        event.accepted = true
                    } else if (event.key === Qt.Key_PageDown) {
                        selectRow(Math.min(count - 1, (i < 0 ? 0 : i) + pageStep()))
                        event.accepted = true
                    } else if (event.key === Qt.Key_PageUp) {
                        selectRow(Math.max(0, (i < 0 ? 0 : i) - pageStep()))
                        event.accepted = true
                    } else if (event.key === Qt.Key_Backspace) {
                        if (controller.canGoUp && !controller.loading)
                            controller.goUp()
                        event.accepted = true
                    } else if (i >= 0
                               && (event.key === Qt.Key_Return
                                   || event.key === Qt.Key_Enter)) {
                        controller.activateToken(controller.entryToken(i))
                        event.accepted = true
                    } else if (i >= 0 && event.key === Qt.Key_Space) {
                        controller.selectToken(controller.entryToken(i))
                        event.accepted = true
                    } else if (event.modifiers === Qt.NoModifier
                               && event.text.length === 1
                               && event.text !== " "
                               && event.text >= " ") {
                        // type-ahead: jump to the next entry starting with the char
                        const ch = event.text.toLowerCase()
                        const start = i < 0 ? -1 : i
                        for (let k = 1; k <= count; k++) {
                            const j = (start + k) % count
                            const name = controller.entryNames[j]
                            if (name && name.toLowerCase().indexOf(ch) === 0) {
                                selectRow(j)
                                break
                            }
                        }
                        event.accepted = true
                    }
                }

                delegate: Item {
                    id: row

                    required property int index
                    required property string modelData

                    readonly property int revision: controller.viewRevision
                    readonly property string token: revision >= 0
                                                    ? controller.entryToken(index)
                                                    : ""
                    readonly property string kind: revision >= 0
                                                   ? controller.entryKind(index)
                                                   : ""
                    readonly property string subtitle: revision >= 0
                                                       ? controller.entrySubtitle(index)
                                                       : ""
                    readonly property bool selected: controller.selectedToken === token

                    width: fileList.width
                    height: CelestinaTheme.rowHeight
                    Accessible.role: Accessible.ListItem
                    Accessible.name: modelData
                    Accessible.selected: selected

                    Rectangle {
                        anchors.fill: parent
                        anchors.leftMargin: 4
                        anchors.rightMargin: 4
                        radius: CelestinaTheme.radiusSm
                        color: row.selected
                               ? CelestinaTheme.surfaceSelected
                               : pointer.containsMouse
                                 ? CelestinaTheme.surfaceHover
                                 : "transparent"
                        border.width: row.selected ? 1 : 0
                        border.color: CelestinaTheme.borderStrong

                        Behavior on color {
                            ColorAnimation {
                                duration: CelestinaTheme.motionFast
                            }
                        }
                    }

                    Rectangle {
                        id: kindGlyph
                        x: 14
                        anchors.verticalCenter: parent.verticalCenter
                        width: CelestinaTheme.glyphTile
                        height: CelestinaTheme.glyphTile
                        radius: CelestinaTheme.radiusSm
                        color: row.kind === "directory"
                               ? CelestinaTheme.glyphDirectory
                               : row.kind === "symlink"
                                 ? CelestinaTheme.glyphSymlink
                                 : CelestinaTheme.glyphFile

                        IconImage {
                            anchors.centerIn: parent
                            width: CelestinaTheme.iconMd
                            height: CelestinaTheme.iconMd
                            name: row.kind === "directory"
                                  ? "folder"
                                  : row.kind === "symlink"
                                    ? "emblem-symbolic-link"
                                    : "text-x-generic"
                            source: CelestinaTheme.fallbackIcon(
                                        row.kind === "directory"
                                        ? "folder"
                                        : row.kind === "symlink"
                                          ? "symlink"
                                          : "file")
                            color: row.kind === "directory"
                                   ? CelestinaTheme.accent
                                   : CelestinaTheme.textMuted
                        }
                    }

                    Text {
                        x: kindGlyph.x + kindGlyph.width + 12
                        y: 9
                        width: parent.width - x - 24
                        text: row.modelData
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontBody
                        font.weight: CelestinaTheme.weightMedium
                        elide: Text.ElideMiddle
                    }

                    Text {
                        x: kindGlyph.x + kindGlyph.width + 12
                        y: 30
                        width: parent.width - x - 24
                        text: row.subtitle
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                        elide: Text.ElideRight
                    }

                    MouseArea {
                        id: pointer
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        hoverEnabled: true

                        onClicked: function(mouse) {
                            fileList.forceActiveFocus()
                            fileList.currentIndex = row.index
                            controller.selectToken(row.token)
                            if (mouse.button === Qt.RightButton) {
                                const point = row.mapToItem(
                                                window.contentItem,
                                                mouse.x, mouse.y)
                                entryMenu.targetToken = row.token
                                entryMenu.targetName = row.modelData
                                entryMenu.targetDirectory =
                                        controller.entryIsDirectory(row.index)
                                entryMenu.popup(window.contentItem, point)
                            }
                        }

                        onDoubleClicked: function(mouse) {
                            if (mouse.button === Qt.LeftButton)
                                controller.activateToken(row.token)
                        }
                    }
                }

                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                }
            }

            GridView {
                id: fileGrid
                x: 8
                y: 60
                width: parent.width - 16
                height: parent.height - 100
                visible: mainPanel.viewMode === "grid"
                model: controller.entryNames
                clip: true
                cellWidth: 132
                cellHeight: 112
                cacheBuffer: 480
                boundsBehavior: Flickable.StopAtBounds
                currentIndex: -1

                delegate: Item {
                    id: cell

                    required property int index
                    required property string modelData

                    readonly property int revision: controller.viewRevision
                    readonly property string token: revision >= 0
                                                    ? controller.entryToken(index)
                                                    : ""
                    readonly property string kind: revision >= 0
                                                   ? controller.entryKind(index)
                                                   : ""
                    readonly property bool selected: controller.selectedToken === token

                    width: fileGrid.cellWidth
                    height: fileGrid.cellHeight
                    Accessible.role: Accessible.ListItem
                    Accessible.name: modelData
                    Accessible.selected: selected

                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: 5
                        radius: CelestinaTheme.radiusSm
                        color: cell.selected
                               ? CelestinaTheme.surfaceSelected
                               : cellMouse.containsMouse
                                 ? CelestinaTheme.surfaceHover
                                 : "transparent"
                        border.width: cell.selected ? 1 : 0
                        border.color: CelestinaTheme.borderStrong

                        Behavior on color {
                            ColorAnimation {
                                duration: CelestinaTheme.motionFast
                            }
                        }
                    }

                    Column {
                        anchors.centerIn: parent
                        spacing: 8

                        Rectangle {
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: 48
                            height: 48
                            radius: CelestinaTheme.radiusSm
                            color: cell.kind === "directory"
                                   ? CelestinaTheme.glyphDirectory
                                   : cell.kind === "symlink"
                                     ? CelestinaTheme.glyphSymlink
                                     : CelestinaTheme.glyphFile

                            IconImage {
                                anchors.centerIn: parent
                                width: 26
                                height: 26
                                name: cell.kind === "directory"
                                      ? "folder"
                                      : cell.kind === "symlink"
                                        ? "emblem-symbolic-link"
                                        : "text-x-generic"
                                source: CelestinaTheme.fallbackIcon(
                                            cell.kind === "directory"
                                            ? "folder"
                                            : cell.kind === "symlink"
                                              ? "symlink"
                                              : "file")
                                color: cell.kind === "directory"
                                       ? CelestinaTheme.accent
                                       : CelestinaTheme.textMuted
                            }
                        }

                        Text {
                            width: fileGrid.cellWidth - 22
                            horizontalAlignment: Text.AlignHCenter
                            text: cell.modelData
                            color: CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontCaption
                            elide: Text.ElideRight
                            maximumLineCount: 2
                            wrapMode: Text.Wrap
                        }
                    }

                    MouseArea {
                        id: cellMouse
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        hoverEnabled: true

                        onClicked: function(mouse) {
                            fileGrid.forceActiveFocus()
                            fileGrid.currentIndex = cell.index
                            controller.selectToken(cell.token)
                            if (mouse.button === Qt.RightButton) {
                                const point = cell.mapToItem(
                                                window.contentItem,
                                                mouse.x, mouse.y)
                                entryMenu.targetToken = cell.token
                                entryMenu.targetName = cell.modelData
                                entryMenu.targetDirectory =
                                        controller.entryIsDirectory(cell.index)
                                entryMenu.popup(window.contentItem, point)
                            }
                        }

                        onDoubleClicked: function(mouse) {
                            if (mouse.button === Qt.LeftButton)
                                controller.activateToken(cell.token)
                        }
                    }
                }

                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                }
            }

            Column {
                anchors.centerIn: fileList
                spacing: 8
                visible: !controller.loading
                         && controller.errorText.length === 0
                         && controller.entryNames.length === 0

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: controller.query.length > 0 ? "Sin coincidencias" : "Carpeta vacía"
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontTitle
                    font.weight: CelestinaTheme.weightMedium
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: controller.query.length > 0
                          ? "Prueba con otra búsqueda."
                          : "No hay elementos que mostrar."
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontLabel
                }
            }

            Rectangle {
                id: errorBanner
                x: 16
                y: 62
                width: parent.width - 32
                height: errorText.implicitHeight + 22
                radius: CelestinaTheme.radiusSm
                visible: controller.errorText.length > 0
                color: CelestinaTheme.dangerFill
                border.width: 1
                border.color: CelestinaTheme.dangerBorder
                z: 3

                Text {
                    id: errorText
                    anchors.fill: parent
                    anchors.margins: 11
                    text: controller.errorText
                    color: CelestinaTheme.dangerText
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontLabel
                    wrapMode: Text.Wrap
                }
            }

            Text {
                id: statusLine
                x: 18
                y: parent.height - height - 13
                width: Math.max(0, countLabel.x - x - 12)
                text: controller.statusText
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                elide: Text.ElideRight
            }

            Text {
                id: countLabel
                anchors.right: parent.right
                anchors.rightMargin: 18
                y: parent.height - height - 13
                text: controller.entryNames.length
                      + (controller.entryNames.length === 1
                         ? " elemento" : " elementos")
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
            }
        }
    }

    GlassContextMenu {
        id: sortMenu
        backdropSource: contentLayer

        GlassMenuItem {
            text: "Nombre"
            current: controller.sortField === 0
            onTriggered: controller.changeSortField(0)
        }

        GlassMenuItem {
            text: "Tamaño"
            current: controller.sortField === 1
            onTriggered: controller.changeSortField(1)
        }

        GlassMenuItem {
            text: "Fecha de modificación"
            current: controller.sortField === 2
            onTriggered: controller.changeSortField(2)
        }

        GlassMenuItem {
            text: "Tipo"
            current: controller.sortField === 3
            onTriggered: controller.changeSortField(3)
        }
    }

    GlassContextMenu {
        id: entryMenu
        backdropSource: contentLayer

        property string targetToken: ""
        property string targetName: ""
        property bool targetDirectory: false

        GlassMenuItem {
            text: entryMenu.targetDirectory ? "Abrir carpeta" : "Seleccionar archivo"
            icon.name: entryMenu.targetDirectory ? "folder-open" : "text-x-generic"
            icon.source: CelestinaTheme.fallbackIcon(
                             entryMenu.targetDirectory ? "folder" : "file")
            onTriggered: controller.activateToken(entryMenu.targetToken)
        }

        MenuSeparator {
            contentItem: Rectangle {
                implicitHeight: 1
                color: CelestinaTheme.border
            }
        }

        GlassMenuItem {
            text: "Actualizar"
            icon.name: "view-refresh"
            icon.source: CelestinaTheme.fallbackIcon("view-refresh")
            onTriggered: controller.refresh()
        }

        GlassMenuItem {
            text: controller.showHidden
                  ? "Ocultar elementos ocultos"
                  : "Mostrar elementos ocultos"
            onTriggered: controller.toggleHidden()
        }
    }

    Component.onCompleted: controller.start()
}
