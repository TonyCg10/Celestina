import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// ─── TopBar ─────────────────────────────────────────────────────────────────
// Migas de pan (editable como campo de ruta) + campo de búsqueda, cada uno una
// pastilla que se vela a cristal cuando el contenido pasa por debajo. Es un hub:
// expone `activeView` / `floating` / `glassTick` que consumen la tira de
// pestañas, las cabeceras y los bordes de scroll, y `searchText` / `beginEditing`
// / `focusSearch` para los atajos de la vista. El controlador, la vista activa,
// la ventana, dónde abrir menús y el menú de ruta llegan por propiedad; devolver
// el foco a la lista sale como señal.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    property var controller
    property Item activeView    // fileList o fileGrid según el modo (lo fija quien instancia)
    property var hostWindow
    property Item overlayParent
    property var pathMenu

    // Scroll offset of the active view (0 at the very top).
    readonly property real scrollY: root.activeView
                                    ? root.activeView.contentY + root.activeView.topMargin : 0
    // Once scrolled, each independent pill fades to glass in place.
    readonly property bool floating: root.scrollY > 6

    // Pulsed each time the pills refresh their capture, so the floating tab
    // pills below refresh their glass in the same beat.
    signal glassTick()
    // The search field lives here; the folder view reads / clears it.
    property alias searchText: searchField.text
    // Returning focus to the list is the folder view's call (it owns fileList).
    signal viewFocusRequested()

    function beginEditing() { pathPill.beginEditing() }
    function focusSearch() {
        searchField.forceActiveFocus()
        searchField.selectAll()
    }

    function refreshGlass() {
        pathGlass.refreshBackdrop()
        searchGlass.refreshBackdrop()
        root.glassTick()
    }

    onFloatingChanged: if (floating) Qt.callLater(root.refreshGlass)

    // Refresh the blur as content scrolls under the pills; work stops when
    // scrolling stops (no continuous work at rest).
    Connections {
        target: root.activeView
        function onContentYChanged() {
            if (root.floating)
                root.refreshGlass()
        }
    }

    Rectangle {
        id: pathPill

        property bool editing: false

        function beginEditing() {
            editing = true
            locationField.text = root.controller.currentPath
            locationField.forceActiveFocus()
            locationField.selectAll()
        }

        function cancelEditing() {
            editing = false
            root.viewFocusRequested()
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
        border.color: root.floating ? "transparent" : CelestinaTheme.inputBorder

        GlassSurface {
            id: pathGlass
            anchors.fill: parent
            backdropSource: root.activeView
            captureEnabled: root.floating
            cornerRadius: parent.radius
            opacity: root.floating ? 1 : 0
            Behavior on opacity {
                NumberAnimation { duration: CelestinaTheme.motionNormal }
            }
        }

        MouseArea {
            id: pathMouse
            anchors.fill: parent
            visible: !pathPill.editing
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            cursorShape: Qt.IBeamCursor
            Accessible.name: "Editar ubicación"
            onClicked: function(mouse) {
                if (mouse.button === Qt.RightButton) {
                    const point = pathMouse.mapToItem(
                                    root.overlayParent, mouse.x, mouse.y)
                    root.pathMenu.popup(root.overlayParent, point)
                } else {
                    pathPill.beginEditing()
                }
            }
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
                model: pathPill.pathSegments(root.controller.currentPath)

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
                        font.pixelSize: Math.round(CelestinaTheme.fontLabel * root.hostWindow.interfaceTextScale)
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
                            font.pixelSize: Math.round(CelestinaTheme.fontLabel * root.hostWindow.interfaceTextScale)
                        }

                        MouseArea {
                            id: crumbMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.controller.openLocation(
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
            font.pixelSize: Math.round(CelestinaTheme.fontLabel * root.hostWindow.interfaceTextScale)
            background: null
            Accessible.name: "Ubicación"

            onActiveFocusChanged: {
                if (!activeFocus && pathPill.editing)
                    pathPill.editing = false
            }

            onAccepted: {
                const location = text
                pathPill.editing = false
                root.controller.openLocation(location)
                root.viewFocusRequested()
            }

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    pathPill.cancelEditing()
                    event.accepted = true
                }
            }
        }

        Connections {
            target: root.controller

            function onCurrentPathChanged() {
                if (pathPill.editing)
                    pathPill.editing = false
            }
        }
    }

    TextField {
        id: searchField
        // Flexes with the interface text scale — the field grows and the
        // breadcrumb (which fills the rest) yields space — so a larger
        // search text is never clipped.
        width: Math.round(Math.min(root.width * 0.42,
                                   Math.max(190, 180 * root.hostWindow.interfaceTextScale)))
        height: CelestinaTheme.controlHeight
        x: root.width - width - 14
        anchors.verticalCenter: parent.verticalCenter
        placeholderText: "Buscar aquí y en subcarpetas"
        color: CelestinaTheme.text
        placeholderTextColor: CelestinaTheme.textMuted
        selectionColor: CelestinaTheme.accentStrong
        selectedTextColor: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.interfaceTextScale)
        leftPadding: 13
        rightPadding: 13
        // Typing always searches — a recursive walk grouped into "in this
        // folder" and "in subfolders"; clearing it exits search.
        onTextEdited: searchDebounce.restart()
        onAccepted: if (text.trim().length > 0)
                        root.controller.searchRecursive(text)

        background: Item {
            Rectangle {
                anchors.fill: parent
                radius: CelestinaTheme.radiusSm
                color: searchField.activeFocus
                       ? CelestinaTheme.inputFillFocus
                       : CelestinaTheme.inputFill
                border.width: 1
                border.color: root.floating
                              ? "transparent"
                              : searchField.activeFocus
                                ? CelestinaTheme.focus
                                : CelestinaTheme.inputBorder
            }
            GlassSurface {
                id: searchGlass
                anchors.fill: parent
                backdropSource: root.activeView
                captureEnabled: root.floating
                cornerRadius: CelestinaTheme.radiusSm
                opacity: root.floating ? 1 : 0
                Behavior on opacity {
                    NumberAnimation { duration: CelestinaTheme.motionNormal }
                }
            }
        }
    }

    Timer {
        id: searchDebounce
        interval: 220
        repeat: false
        onTriggered: {
            if (searchField.text.trim().length > 0)
                root.controller.searchRecursive(searchField.text)
            else
                root.controller.closeSearch()
        }
    }
}
