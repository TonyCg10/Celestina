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

        // Floating ghost shown while dragging a folder toward the sidebar.
        Item {
            id: dragGhost
            z: 100
            width: 150
            height: 34
            visible: Drag.active
            property string path: ""
            property string label: ""
            Drag.keys: ["siderita-bookmark"]
            Drag.hotSpot.x: 16
            Drag.hotSpot.y: 17

            Rectangle {
                anchors.fill: parent
                radius: CelestinaTheme.radiusSm
                color: CelestinaTheme.surfaceStrong
                border.width: 1
                border.color: CelestinaTheme.accent
                opacity: 0.96

                Row {
                    x: 10
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 8

                    IconImage {
                        anchors.verticalCenter: parent.verticalCenter
                        width: CelestinaTheme.iconSm
                        height: CelestinaTheme.iconSm
                        name: "folder"
                        source: CelestinaTheme.fallbackIcon("folder")
                        color: CelestinaTheme.accent
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 104
                        text: dragGhost.label
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontLabel
                        elide: Text.ElideRight
                    }
                }
            }
        }

        Item {
            id: topBar
            z: 10
            x: mainPanel.x + 12
            y: mainPanel.y + 12
            width: mainPanel.width - 24
            height: 52

            // Scroll offset of the active view (0 at the very top).
            readonly property real scrollY: mainPanel.viewMode === "grid"
                                            ? fileGrid.contentY + fileGrid.topMargin
                                            : fileList.contentY + fileList.topMargin
            // Once scrolled, each independent pill fades to glass in place.
            readonly property bool floating: scrollY > 6
            readonly property Item activeView: mainPanel.viewMode === "grid"
                                               ? fileGrid : fileList

            function refreshGlass() {
                pathGlass.refreshBackdrop()
                searchGlass.refreshBackdrop()
            }

            onFloatingChanged: if (floating) Qt.callLater(topBar.refreshGlass)

            // Refresh the blur as content scrolls under the pills; work stops
            // when scrolling stops (no continuous work at rest).
            Connections {
                target: topBar.activeView
                function onContentYChanged() {
                    if (topBar.floating)
                        topBar.refreshGlass()
                }
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

                x: 14
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(180, searchField.x - x - 12)
                height: CelestinaTheme.controlHeight
                radius: CelestinaTheme.radiusSm
                clip: true
                color: CelestinaTheme.inputFill
                border.width: 1
                border.color: topBar.floating ? "transparent" : CelestinaTheme.inputBorder

                GlassSurface {
                    id: pathGlass
                    anchors.fill: parent
                    backdropSource: topBar.activeView
                    captureEnabled: topBar.floating
                    cornerRadius: parent.radius
                    opacity: topBar.floating ? 1 : 0
                    Behavior on opacity {
                        NumberAnimation { duration: CelestinaTheme.motionNormal }
                    }
                }

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
                x: topBar.width - width - 14
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

                background: Item {
                    Rectangle {
                        anchors.fill: parent
                        radius: CelestinaTheme.radiusSm
                        color: searchField.activeFocus
                               ? CelestinaTheme.inputFillFocus
                               : CelestinaTheme.inputFill
                        border.width: 1
                        border.color: topBar.floating
                                      ? "transparent"
                                      : searchField.activeFocus
                                        ? CelestinaTheme.focus
                                        : CelestinaTheme.inputBorder
                    }
                    GlassSurface {
                        id: searchGlass
                        anchors.fill: parent
                        backdropSource: topBar.activeView
                        captureEnabled: topBar.floating
                        cornerRadius: CelestinaTheme.radiusSm
                        opacity: topBar.floating ? 1 : 0
                        Behavior on opacity {
                            NumberAnimation { duration: CelestinaTheme.motionNormal }
                        }
                    }
                }
            }

            Timer {
                id: searchDebounce
                interval: 120
                repeat: false
                onTriggered: controller.applyQuery(searchField.text)
            }

        }

        Rectangle {
            id: sidebar
            x: 20
            y: 18
            width: 184
            height: parent.height - y - 20
            radius: CelestinaTheme.radiusLg
            visible: parent.width >= 820
            color: CelestinaTheme.surface
            border.width: 1
            border.color: CelestinaTheme.border

            DropArea {
                anchors.fill: parent
                keys: ["siderita-bookmark"]
                onDropped: controller.addBookmark(dragGhost.path)

                Rectangle {
                    anchors.fill: parent
                    radius: sidebar.radius
                    color: "transparent"
                    border.width: 2
                    border.color: CelestinaTheme.accent
                    visible: parent.containsDrag
                }
            }

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

            Column {
                id: placesColumn
                x: 8
                y: 42
                width: parent.width - 16
                spacing: 2

                Repeater {
                    model: [
                        { name: "Inicio", icon: "user-home", key: "HOME", fallback: "go-home" },
                        { name: "Escritorio", icon: "user-desktop", key: "DESKTOP", fallback: "folder" },
                        { name: "Documentos", icon: "folder-documents", key: "DOCUMENTS", fallback: "folder" },
                        { name: "Descargas", icon: "folder-download", key: "DOWNLOAD", fallback: "folder" },
                        { name: "Música", icon: "folder-music", key: "MUSIC", fallback: "folder" },
                        { name: "Imágenes", icon: "folder-pictures", key: "PICTURES", fallback: "folder" },
                        { name: "Vídeos", icon: "folder-videos", key: "VIDEOS", fallback: "folder" }
                    ]

                    delegate: Item {
                        id: placeRow

                        required property var modelData

                        readonly property string placePath: controller.placePath(modelData.key)
                        readonly property bool available: placePath.length > 0
                        readonly property bool current: available
                                                        && placePath === controller.currentPath

                        visible: available
                        height: available ? 34 : 0
                        width: placesColumn.width

                        Rectangle {
                            anchors.fill: parent
                            anchors.leftMargin: 2
                            anchors.rightMargin: 2
                            radius: CelestinaTheme.radiusSm
                            color: placeRow.current
                                   ? CelestinaTheme.badgeAccentFill
                                   : placeMouse.containsMouse
                                     ? CelestinaTheme.surfaceHover
                                     : "transparent"
                        }

                        IconImage {
                            id: placeIcon
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: CelestinaTheme.iconSm
                            height: CelestinaTheme.iconSm
                            name: placeRow.modelData.icon
                            source: CelestinaTheme.fallbackIcon(placeRow.modelData.fallback)
                            color: placeRow.current ? CelestinaTheme.accent
                                                    : CelestinaTheme.textMuted
                        }

                        Text {
                            x: placeIcon.x + placeIcon.width + 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - x - 12
                            text: placeRow.modelData.name
                            color: placeRow.current ? CelestinaTheme.accent
                                                    : CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontLabel
                            font.weight: placeRow.current ? CelestinaTheme.weightMedium
                                                          : CelestinaTheme.weightRegular
                            elide: Text.ElideRight
                        }

                        MouseArea {
                            id: placeMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: controller.openLocation(placeRow.placePath)
                        }
                    }
                }
            }

            Text {
                id: bookmarksLabel
                x: 16
                y: placesColumn.y + placesColumn.height + 12
                text: "MARCADORES"
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
                font.letterSpacing: 1.4
                font.weight: CelestinaTheme.weightDemiBold
            }

            ListView {
                id: bookmarksList
                x: 8
                y: bookmarksLabel.y + 20
                width: parent.width - 16
                height: Math.max(0, readOnlyBadge.y - y - 12)
                clip: true
                model: controller.bookmarkNames
                spacing: 2
                boundsBehavior: Flickable.StopAtBounds

                property int editIndex: -1

                delegate: Item {
                    id: bmRow

                    required property int index
                    required property string modelData

                    readonly property string path: (index >= 0
                            && index < controller.bookmarkPaths.length)
                            ? controller.bookmarkPaths[index] : ""
                    readonly property bool current: path === controller.currentPath
                    readonly property bool editing: bookmarksList.editIndex === index

                    width: bookmarksList.width
                    height: 34

                    Rectangle {
                        anchors.fill: parent
                        anchors.leftMargin: 2
                        anchors.rightMargin: 2
                        radius: CelestinaTheme.radiusSm
                        color: bmRow.current
                               ? CelestinaTheme.badgeAccentFill
                               : bmMouse.containsMouse
                                 ? CelestinaTheme.surfaceHover
                                 : "transparent"
                    }

                    IconImage {
                        id: bmIcon
                        x: 12
                        anchors.verticalCenter: parent.verticalCenter
                        width: CelestinaTheme.iconSm
                        height: CelestinaTheme.iconSm
                        name: "folder"
                        source: CelestinaTheme.fallbackIcon("folder")
                        color: bmRow.current ? CelestinaTheme.accent
                                             : CelestinaTheme.textMuted
                    }

                    Text {
                        visible: !bmRow.editing
                        x: bmIcon.x + bmIcon.width + 10
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - x - 12
                        text: bmRow.modelData
                        color: bmRow.current ? CelestinaTheme.accent
                                             : CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontLabel
                        font.weight: bmRow.current ? CelestinaTheme.weightMedium
                                                   : CelestinaTheme.weightRegular
                        elide: Text.ElideRight
                    }

                    TextField {
                        id: bmField
                        visible: bmRow.editing
                        x: bmIcon.x + bmIcon.width + 6
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - x - 8
                        height: 26
                        text: bmRow.modelData
                        color: CelestinaTheme.text
                        selectionColor: CelestinaTheme.accentStrong
                        selectedTextColor: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontLabel
                        leftPadding: 8
                        rightPadding: 8
                        background: Rectangle {
                            radius: CelestinaTheme.radiusXs
                            color: CelestinaTheme.inputFillFocus
                            border.width: 1
                            border.color: CelestinaTheme.focus
                        }
                        onVisibleChanged: if (visible) { forceActiveFocus(); selectAll() }
                        onAccepted: {
                            controller.renameBookmark(bmRow.index, text)
                            bookmarksList.editIndex = -1
                        }
                        onActiveFocusChanged: {
                            if (!activeFocus && bookmarksList.editIndex === bmRow.index)
                                bookmarksList.editIndex = -1
                        }
                        Keys.onPressed: function(event) {
                            if (event.key === Qt.Key_Escape) {
                                bookmarksList.editIndex = -1
                                event.accepted = true
                            }
                        }
                    }

                    MouseArea {
                        id: bmMouse
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: function(mouse) {
                            if (mouse.button === Qt.RightButton) {
                                const point = bmRow.mapToItem(window.contentItem,
                                                              mouse.x, mouse.y)
                                bmMenu.targetIndex = bmRow.index
                                bmMenu.popup(window.contentItem, point)
                            } else {
                                controller.openLocation(bmRow.path)
                            }
                        }
                        onDoubleClicked: bookmarksList.editIndex = bmRow.index
                    }
                }
            }

            Rectangle {
                id: readOnlyBadge
                x: 14
                y: sidebarInfo.y - height - 12
                width: parent.width - 28
                height: 32
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

            // Bottom info box: folder count now, room for more later.
            Rectangle {
                id: sidebarInfo
                x: 14
                width: parent.width - 28
                y: parent.height - height - 14
                height: 62
                radius: CelestinaTheme.radiusSm
                color: CelestinaTheme.badgeFill
                border.width: 1
                border.color: CelestinaTheme.border

                Column {
                    x: 14
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 28
                    spacing: 4

                    Text {
                        text: "CARPETA"
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontMini
                        font.letterSpacing: 1.4
                        font.weight: CelestinaTheme.weightDemiBold
                    }

                    Text {
                        text: controller.entryNames.length
                              + (controller.entryNames.length === 1
                                 ? " elemento" : " elementos")
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontBody
                        font.weight: CelestinaTheme.weightMedium
                    }
                }
            }
        }

        Rectangle {
            id: mainPanel

            property string viewMode: "list"   // "list" | "grid"
            property real itemScale: 1.0        // view zoom: row/cell + icon size

            readonly property int listRowHeight: Math.round(CelestinaTheme.rowHeight * itemScale)
            readonly property int gridCellWidth: Math.round(132 * itemScale)
            readonly property int gridCellHeight: Math.round(112 * itemScale)

            // ── Multi-selection (token-keyed, so it survives sort/filter) ────
            property var selectedTokens: ({})
            property int selectionCount: 0
            property string anchorToken: ""

            function isSelected(token) {
                return token.length > 0 && selectedTokens[token] === true
            }
            function clearSelection() {
                selectedTokens = ({})
                selectionCount = 0
            }
            function selectOnly(token) {
                var s = {}
                s[token] = true
                selectedTokens = s
                selectionCount = 1
                anchorToken = token
            }
            function toggleSelection(token) {
                var s = Object.assign({}, selectedTokens)
                if (s[token])
                    delete s[token]
                else
                    s[token] = true
                selectedTokens = s
                selectionCount = Object.keys(s).length
                anchorToken = token
            }
            function selectRange(toIndex) {
                var anchorIdx = controller.indexForToken(anchorToken)
                if (anchorIdx < 0)
                    anchorIdx = toIndex
                var s = {}
                for (var i = Math.min(anchorIdx, toIndex);
                     i <= Math.max(anchorIdx, toIndex); i++) {
                    var t = controller.entryToken(i)
                    if (t.length > 0)
                        s[t] = true
                }
                selectedTokens = s
                selectionCount = Object.keys(s).length
            }
            function selectAll() {
                var s = {}
                for (var i = 0; i < controller.entryNames.length; i++) {
                    var t = controller.entryToken(i)
                    if (t.length > 0)
                        s[t] = true
                }
                selectedTokens = s
                selectionCount = Object.keys(s).length
            }
            // Selection = base plus every item sampled inside the marquee rect
            // (x0,y0..x1,y1 in the view's viewport coordinates).
            function selectRectFrom(base, view, x0, y0, x1, y1) {
                var s = Object.assign({}, base)
                var cx = view.contentX
                var cy = view.contentY
                for (var y = y0; y <= y1; y += 10) {
                    for (var x = x0; x <= x1; x += 24) {
                        var idx = view.indexAt(x + cx, y + cy)
                        if (idx >= 0) {
                            var t = controller.entryToken(idx)
                            if (t.length > 0)
                                s[t] = true
                        }
                    }
                }
                selectedTokens = s
                selectionCount = Object.keys(s).length
            }

            Connections {
                target: controller
                function onCurrentPathChanged() { mainPanel.clearSelection() }
            }

            x: sidebar.visible ? sidebar.x + sidebar.width + 14 : 20
            y: 18
            width: parent.width - x - 20
            height: parent.height - y - 20
            radius: CelestinaTheme.radiusLg
            color: CelestinaTheme.surface
            border.width: 1
            border.color: CelestinaTheme.border

            // Bottom control bar: all controls and status render along the
            // bottom of the content box; the list/grid fill from the top.
            Item {
                id: bottomBar
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 54

                Rectangle {
                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.leftMargin: 16
                    anchors.rightMargin: 16
                    height: 1
                    color: CelestinaTheme.border
                }
            }

            Rectangle {
                id: hiddenToggle
                x: 16
                anchors.verticalCenter: bottomBar.verticalCenter
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
                width: 26
                height: 26
                x: viewToggle.x + viewToggle.width + 12
                anchors.verticalCenter: bottomBar.verticalCenter
                running: controller.loading
                visible: running
            }

            NavButton {
                id: sortDirectionButton
                x: sortButton.x + sortButton.width + 8
                anchors.verticalCenter: bottomBar.verticalCenter
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

                x: hiddenToggle.x + hiddenToggle.width + 12
                anchors.verticalCenter: bottomBar.verticalCenter
                width: 116
                height: 34
                text: labels[controller.sortField]
                Accessible.name: "Ordenar por " + text
                onClicked: {
                    // Button is at the bottom now — open the menu upward.
                    // sortMenu.height can be 0 before the first open; fall back
                    // to an estimate for the four sort options.
                    const menuHeight = sortMenu.height > 0 ? sortMenu.height : 172
                    const point = sortButton.mapToItem(
                                    window.contentItem, 0, -menuHeight - 6)
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

                x: sortDirectionButton.x + sortDirectionButton.width + 10
                anchors.verticalCenter: bottomBar.verticalCenter
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

            // Right-click on empty space (behind the views) opens the folder
            // menu; right-clicks that land on an item are handled by the item.
            MouseArea {
                id: emptySpaceMouse
                x: 8
                y: 14
                width: parent.width - 16
                height: parent.height - 68
                acceptedButtons: Qt.RightButton
                onClicked: function(mouse) {
                    const point = emptySpaceMouse.mapToItem(
                                    window.contentItem, mouse.x, mouse.y)
                    folderMenu.popup(window.contentItem, point)
                }
            }

            ListView {
                id: fileList
                x: 8
                y: 14
                width: parent.width - 16
                height: parent.height - 68
                visible: mainPanel.viewMode === "list"
                model: controller.entryNames
                clip: true
                spacing: 2
                reuseItems: true
                cacheBuffer: 420
                topMargin: 62
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
                    const t = controller.entryToken(i)
                    mainPanel.selectOnly(t)
                    controller.selectToken(t)
                    positionViewAtIndex(i, ListView.Contain)
                }

                function pageStep() {
                    return Math.max(
                        1, Math.floor(height / (mainPanel.listRowHeight + spacing)))
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
                    readonly property bool selected: mainPanel.isSelected(token)

                    width: fileList.width
                    height: mainPanel.listRowHeight
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
                        width: Math.round(CelestinaTheme.glyphTile * mainPanel.itemScale)
                        height: Math.round(CelestinaTheme.glyphTile * mainPanel.itemScale)
                        radius: CelestinaTheme.radiusSm
                        color: row.kind === "directory"
                               ? CelestinaTheme.glyphDirectory
                               : row.kind === "symlink"
                                 ? CelestinaTheme.glyphSymlink
                                 : CelestinaTheme.glyphFile

                        IconImage {
                            anchors.centerIn: parent
                            width: Math.round(CelestinaTheme.iconMd * mainPanel.itemScale)
                            height: Math.round(CelestinaTheme.iconMd * mainPanel.itemScale)
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

                    Column {
                        id: rowText
                        x: kindGlyph.x + kindGlyph.width + 12
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - x - 24
                        spacing: 1

                        Text {
                            width: parent.width
                            text: row.modelData
                            color: CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * mainPanel.itemScale)
                            font.weight: CelestinaTheme.weightMedium
                            elide: Text.ElideMiddle
                        }

                        Text {
                            width: parent.width
                            text: row.subtitle
                            color: CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontCaption * mainPanel.itemScale)
                            elide: Text.ElideRight
                        }
                    }

                    MouseArea {
                        id: pointer
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        hoverEnabled: true

                        onClicked: function(mouse) {
                            fileList.forceActiveFocus()
                            fileList.currentIndex = row.index
                            if (mouse.button === Qt.RightButton) {
                                if (!mainPanel.isSelected(row.token))
                                    mainPanel.selectOnly(row.token)
                            } else if (mouse.modifiers & Qt.ControlModifier) {
                                mainPanel.toggleSelection(row.token)
                            } else if (mouse.modifiers & Qt.ShiftModifier) {
                                mainPanel.selectRange(row.index)
                            } else {
                                mainPanel.selectOnly(row.token)
                            }
                            controller.selectToken(row.token)
                            if (mouse.button === Qt.RightButton) {
                                const point = row.mapToItem(
                                                window.contentItem,
                                                mouse.x, mouse.y)
                                entryMenu.targetToken = row.token
                                entryMenu.targetName = row.modelData
                                entryMenu.targetDirectory =
                                        controller.entryIsDirectory(row.index)
                                entryMenu.targetPath = controller.entryPath(row.index)
                                entryMenu.popup(window.contentItem, point)
                            }
                        }

                        onDoubleClicked: function(mouse) {
                            if (mouse.button === Qt.LeftButton)
                                controller.activateToken(row.token)
                        }
                    }

                    DragHandler {
                        id: rowDrag
                        target: null
                        dragThreshold: 8
                        enabled: row.revision >= 0
                                 && controller.entryIsDirectory(row.index)
                        onActiveChanged: {
                            if (active) {
                                dragGhost.path = controller.entryPath(row.index)
                                dragGhost.label = row.modelData
                                dragGhost.Drag.active = true
                            } else {
                                dragGhost.Drag.drop()
                                dragGhost.Drag.active = false
                            }
                        }
                        onCentroidChanged: {
                            if (active) {
                                dragGhost.x = centroid.scenePosition.x
                                              - dragGhost.Drag.hotSpot.x
                                dragGhost.y = centroid.scenePosition.y
                                              - dragGhost.Drag.hotSpot.y
                            }
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
                y: 14
                width: parent.width - 16
                height: parent.height - 68
                visible: mainPanel.viewMode === "grid"
                model: controller.entryNames
                clip: true
                cellWidth: mainPanel.gridCellWidth
                cellHeight: mainPanel.gridCellHeight
                cacheBuffer: 480
                topMargin: 62
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
                    readonly property bool selected: mainPanel.isSelected(token)

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
                            width: Math.round(48 * mainPanel.itemScale)
                            height: Math.round(48 * mainPanel.itemScale)
                            radius: CelestinaTheme.radiusSm
                            color: cell.kind === "directory"
                                   ? CelestinaTheme.glyphDirectory
                                   : cell.kind === "symlink"
                                     ? CelestinaTheme.glyphSymlink
                                     : CelestinaTheme.glyphFile

                            IconImage {
                                anchors.centerIn: parent
                                width: Math.round(26 * mainPanel.itemScale)
                                height: Math.round(26 * mainPanel.itemScale)
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
                            font.pixelSize: Math.round(CelestinaTheme.fontCaption * mainPanel.itemScale)
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
                            if (mouse.button === Qt.RightButton) {
                                if (!mainPanel.isSelected(cell.token))
                                    mainPanel.selectOnly(cell.token)
                            } else if (mouse.modifiers & Qt.ControlModifier) {
                                mainPanel.toggleSelection(cell.token)
                            } else if (mouse.modifiers & Qt.ShiftModifier) {
                                mainPanel.selectRange(cell.index)
                            } else {
                                mainPanel.selectOnly(cell.token)
                            }
                            controller.selectToken(cell.token)
                            if (mouse.button === Qt.RightButton) {
                                const point = cell.mapToItem(
                                                window.contentItem,
                                                mouse.x, mouse.y)
                                entryMenu.targetToken = cell.token
                                entryMenu.targetName = cell.modelData
                                entryMenu.targetDirectory =
                                        controller.entryIsDirectory(cell.index)
                                entryMenu.targetPath = controller.entryPath(cell.index)
                                entryMenu.popup(window.contentItem, point)
                            }
                        }

                        onDoubleClicked: function(mouse) {
                            if (mouse.button === Qt.LeftButton)
                                controller.activateToken(cell.token)
                        }
                    }

                    DragHandler {
                        id: cellDrag
                        target: null
                        dragThreshold: 8
                        enabled: cell.revision >= 0
                                 && controller.entryIsDirectory(cell.index)
                        onActiveChanged: {
                            if (active) {
                                dragGhost.path = controller.entryPath(cell.index)
                                dragGhost.label = cell.modelData
                                dragGhost.Drag.active = true
                            } else {
                                dragGhost.Drag.drop()
                                dragGhost.Drag.active = false
                            }
                        }
                        onCentroidChanged: {
                            if (active) {
                                dragGhost.x = centroid.scenePosition.x
                                              - dragGhost.Drag.hotSpot.x
                                dragGhost.y = centroid.scenePosition.y
                                              - dragGhost.Drag.hotSpot.y
                            }
                        }
                    }
                }

                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                }
            }

            // Left-drag on empty space draws a marquee selection zone. Presses
            // that land on an item are passed through to the item's handler.
            MouseArea {
                id: marquee
                x: 8
                y: 14
                width: parent.width - 16
                height: parent.height - 68
                acceptedButtons: Qt.LeftButton
                preventStealing: true

                property bool dragging: false
                property real ox: 0
                property real oy: 0
                property real cx: 0
                property real cy: 0
                property var base: ({})
                readonly property Item view: topBar.activeView

                onPressed: function(mouse) {
                    const idx = view.indexAt(mouse.x + view.contentX,
                                             mouse.y + view.contentY)
                    if (idx >= 0) {
                        mouse.accepted = false   // over an item → item handles it
                        return
                    }
                    fileList.forceActiveFocus()
                    base = (mouse.modifiers & Qt.ControlModifier)
                           ? Object.assign({}, mainPanel.selectedTokens)
                           : {}
                    if (!(mouse.modifiers & Qt.ControlModifier))
                        mainPanel.clearSelection()
                    dragging = false
                    ox = mouse.x; oy = mouse.y
                    cx = mouse.x; cy = mouse.y
                }
                onPositionChanged: function(mouse) {
                    cx = mouse.x; cy = mouse.y
                    if (!dragging && (Math.abs(cx - ox) > 4
                                      || Math.abs(cy - oy) > 4))
                        dragging = true
                    if (dragging)
                        mainPanel.selectRectFrom(marquee.base, marquee.view,
                            Math.min(ox, cx), Math.min(oy, cy),
                            Math.max(ox, cx), Math.max(oy, cy))
                }
                onReleased: function(mouse) { marquee.dragging = false }

                Rectangle {
                    visible: marquee.dragging
                    x: Math.min(marquee.ox, marquee.cx)
                    y: Math.min(marquee.oy, marquee.cy)
                    width: Math.abs(marquee.cx - marquee.ox)
                    height: Math.abs(marquee.cy - marquee.oy)
                    radius: 3
                    color: Qt.rgba(CelestinaTheme.accent.r, CelestinaTheme.accent.g,
                                   CelestinaTheme.accent.b, 0.18)
                    border.width: 1
                    border.color: CelestinaTheme.accent
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
                y: 14
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
                x: busy.x + busy.width + 14
                anchors.verticalCenter: bottomBar.verticalCenter
                width: Math.max(0, zoomSlider.x - x - 12)
                text: mainPanel.selectionCount > 1
                      ? mainPanel.selectionCount + " seleccionados"
                      : controller.statusText
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                elide: Text.ElideRight
            }

            Slider {
                id: zoomSlider

                from: 0.8
                to: 1.9
                value: mainPanel.itemScale
                stepSize: 0.1
                width: 116
                height: 22
                anchors.right: parent.right
                anchors.rightMargin: 16
                anchors.verticalCenter: bottomBar.verticalCenter
                Accessible.name: "Tamaño de los elementos"
                onMoved: mainPanel.itemScale = value

                ToolTip.visible: hovered || pressed
                ToolTip.text: Math.round(value * 100) + " %"

                background: Rectangle {
                    x: zoomSlider.leftPadding
                    y: zoomSlider.topPadding + zoomSlider.availableHeight / 2 - height / 2
                    width: zoomSlider.availableWidth
                    height: 4
                    radius: 2
                    color: CelestinaTheme.controlFill

                    Rectangle {
                        width: zoomSlider.visualPosition * parent.width
                        height: parent.height
                        radius: 2
                        color: CelestinaTheme.accent
                    }
                }

                handle: Rectangle {
                    x: zoomSlider.leftPadding
                       + zoomSlider.visualPosition * (zoomSlider.availableWidth - width)
                    y: zoomSlider.topPadding + zoomSlider.availableHeight / 2 - height / 2
                    width: 15
                    height: 15
                    radius: 7.5
                    color: zoomSlider.pressed ? CelestinaTheme.accent : CelestinaTheme.text
                    border.width: 1
                    border.color: CelestinaTheme.borderStrong
                }
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
        property string targetPath: ""

        GlassMenuItem {
            text: entryMenu.targetDirectory ? "Abrir carpeta" : "Seleccionar archivo"
            icon.name: entryMenu.targetDirectory ? "folder-open" : "text-x-generic"
            icon.source: CelestinaTheme.fallbackIcon(
                             entryMenu.targetDirectory ? "folder" : "file")
            onTriggered: controller.activateToken(entryMenu.targetToken)
        }

        GlassMenuItem {
            text: "Añadir a marcadores"
            visible: entryMenu.targetDirectory
            height: visible ? implicitHeight : 0
            icon.name: "bookmark-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: controller.addBookmark(entryMenu.targetPath)
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

    GlassContextMenu {
        id: folderMenu
        backdropSource: contentLayer

        GlassMenuItem {
            text: "Seleccionar todo"
            onTriggered: mainPanel.selectAll()
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

    GlassContextMenu {
        id: bmMenu
        backdropSource: contentLayer

        property int targetIndex: -1

        GlassMenuItem {
            text: "Renombrar"
            onTriggered: bookmarksList.editIndex = bmMenu.targetIndex
        }

        GlassMenuItem {
            text: "Quitar de marcadores"
            onTriggered: controller.removeBookmark(bmMenu.targetIndex)
        }
    }

    Component.onCompleted: controller.start()
}
