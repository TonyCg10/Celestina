import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// ─── Sidebar ──────────────────────────────────────────────────────────────────
// La columna de la izquierda: lugares, dispositivos, favoritos y marcadores, la
// caja de información de abajo y los cuatro menús contextuales de sus filas. Va
// todo junto porque está todo enlazado — un menú escribe en la lista de
// marcadores para meterla en modo edición, y las filas abren los menús.
//
// Es de la ventana y no de la pestaña: enseña lo que hay en el sistema y lleva
// a la pestaña activa a donde se le pida. De ahí que pida la ventana anfitriona
// (controlador activo, escalas del panel, `openTab`) en vez de tener su propio
// controlador.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    // La ventana anfitriona: controlador activo, escalas del panel, `placeDefs`
    // y `openTab`. Como el panel no navega solo, todo lo que hace acaba en ella.
    property var hostWindow
    // Dónde nacen los menús emergentes, y qué desenfocan por debajo.
    property Item overlayParent
    property Item backdrop
    // El fantasma compartido del arrastre: para saber qué viene cayendo.
    property Item dragGhost

    // Dónde acaba la columna, para que el contenido empiece después. Lo publica
    // el panel porque es él quien decide su ancho, no quien lo coloca.
    readonly property bool panelVisible: sidebar.visible
    readonly property real rightEdge: sidebar.x + sidebar.width

    CelestinaSurface {
        id: sidebar
        x: 20
        y: 18
        width: 184
        // Leave room below for the separate item-info box (its height scales
        // with the sidebar text) plus a gap.
        height: parent.height - y - 18 - sidebarInfo.height - 14
        visible: parent.width >= 820
        role: CelestinaSurface.Panel

        DropArea {
            anchors.fill: parent
            keys: ["siderita-bookmark"]
            onDropped: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.addBookmark(root.dragGhost.path)
            }

            Rectangle {
                anchors.fill: parent
                radius: sidebar.radius
                color: CelestinaTheme.clear
                border.width: CelestinaTheme.borderFocus
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
            font.letterSpacing: CelestinaTheme.sectionLetterSpacing
            font.weight: CelestinaTheme.weightDemiBold
        }

        // Cada zona se pliega por su cabecera. Vive en la sesión y no en
        // disco a propósito: plegar es un gesto de "ahora estorba", no una
        // preferencia — al abrir Siderita el panel enseña lo que hay.
        property bool placesCollapsed: false
        property bool devicesCollapsed: false

        // Todo el sidebar se desplaza: las secciones crecen con lo que el
        // usuario guarda (marcadores, favoritos, dispositivos) y antes la
        // última se comía el espacio de las demás. Una sola barra para el
        // panel entero — las listas de dentro no se desplazan solas.
        Flickable {
            id: sidebarScroll
            x: 0
            y: 42
            width: parent.width
            height: parent.height - y - 10
            clip: true
            contentWidth: width
            contentHeight: savedSections.y + savedSections.height
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

            Column {
                id: placesColumn
                x: 8
                y: 0
                width: parent.width - 16
                spacing: 2

                // La cabecera que faltaba: sin ella los lugares eran la única
                // zona sin nombre y la única que no se podía plegar.
                Item {
                    width: placesColumn.width
                    height: 22

                    SidebarChevron {

                        textScale: root.hostWindow.sidebarTextScale
                        x: -4
                        y: 9
                        collapsed: sidebar.placesCollapsed
                    }

                    CelestinaSectionLabel {
                        x: 8
                        y: 8
                        text: "LUGARES"
                        textScale: root.hostWindow.sidebarTextScale
                    }

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: sidebar.placesCollapsed = !sidebar.placesCollapsed
                    }
                }

                // ── Places ───────────────────────────────────────────────
                // The list, its order and what it leaves out all come from the
                // controller: `placeKeys` is what exists here, arranged the way
                // the user arranged it, minus what they hid. This side only
                // knows how to draw a key.
                ListView {
                    id: placesList
                    width: placesColumn.width
                    height: sidebar.placesCollapsed ? 0 : count * rowPitch
                    visible: !sidebar.placesCollapsed
                    spacing: 2
                    // Short and bounded — the sidebar scrolls, this never does,
                    // and a non-interactive list leaves the wheel to the sidebar.
                    interactive: false
                    model: root.hostWindow.activeController
                           ? root.hostWindow.activeController.placeKeys : []


                    readonly property int rowPitch: root.hostWindow.sidebarRowHeight + spacing
                    property int dragIndex: -1
                    property int dropIndex: -1

                    function moveDragged(from, to) {
                        dragIndex = -1
                        dropIndex = -1
                        // Last: the move republishes placeKeys, which resets this
                        // view and destroys the delegate that called us.
                        if (to >= 0 && to !== from && root.hostWindow.activeController)
                            root.hostWindow.activeController.movePlace(from, to)
                    }

                    delegate: Item {
                        id: placeRow

                        required property int index
                        required property string modelData      // the place key

                        readonly property var def: root.hostWindow.placeDefs[modelData]
                                                   || ({ name: modelData, icon: "folder",
                                                         fallback: "folder" })
                        readonly property bool isTrash: modelData === "TRASH"
                        readonly property bool isRecent: modelData === "RECENT"
                        // Trash and Recientes are locations, not folders: they
                        // have no path to open, they flip a state instead.
                        readonly property string placePath:
                                isTrash || isRecent || !root.hostWindow.activeController
                                ? "" : root.hostWindow.activeController.placePath(modelData)
                        readonly property bool current: isTrash
                                ? (root.hostWindow.activeController
                                   && root.hostWindow.activeController.trashActive)
                                : isRecent
                                ? (root.hostWindow.activeController
                                   && root.hostWindow.activeController.recentActive)
                                : (placePath.length > 0
                                   && placePath === (root.hostWindow.activeController
                                                     ? root.hostWindow.activeController.currentPath : ""))
                        readonly property bool dragging: placesList.dragIndex === index
                        property bool justDragged: false

                        width: placesList.width
                        height: root.hostWindow.sidebarRowHeight
                        z: dragging ? 2 : 0

                        Accessible.role: Accessible.Button
                        Accessible.name: def.name
                        Accessible.onPressAction: placeRow.activate()

                        function activate() {
                            const ac = root.hostWindow.activeController
                            if (!ac)
                                return
                            if (isTrash)
                                ac.openTrash()
                            else if (isRecent)
                                ac.openRecent()
                            else if (placePath.length > 0)
                                ac.openLocation(placePath)
                        }

                        // Where the carried row would land.
                        Rectangle {
                            z: 3
                            visible: placesList.dragIndex >= 0
                                     && placesList.dragIndex !== placeRow.index
                                     && placesList.dropIndex === placeRow.index
                            x: 2
                            width: parent.width - 4
                            height: CelestinaTheme.compDragIndicatorHeight
                            radius: height / 2
                            y: placesList.dropIndex > placesList.dragIndex
                               ? parent.height - height : 0
                            color: CelestinaTheme.accent
                        }

                        Item {
                            id: placeContent
                            width: placeRow.width
                            height: placeRow.height
                            opacity: placeRow.dragging
                                     ? CelestinaTheme.draggedContentOpacity : 1

                            // Eases back into place when a drag ends where it
                            // started; a drag that did move is replaced by the
                            // republished list anyway.
                            Behavior on y {
                                enabled: !placeMouse.drag.active
                                NumberAnimation {
                                    duration: CelestinaTheme.motionFast
                                    easing.type: CelestinaTheme.easeStandard
                                }
                            }

                            Rectangle {
                                anchors.fill: parent
                                anchors.leftMargin: 2
                                anchors.rightMargin: 2
                                radius: CelestinaTheme.radiusSm
                                color: placeRow.dragging
                                       ? CelestinaTheme.surfaceStrong
                                       : placeRow.current
                                         ? CelestinaTheme.badgeAccentFill
                                         : placeMouse.containsMouse
                                           ? CelestinaTheme.surfaceHover
                                           : CelestinaTheme.clear

                                Behavior on color {
                                    ColorAnimation { duration: CelestinaTheme.motionFast }
                                }
                            }

                            CelestinaIcon {
                                id: placeIcon
                                x: 12
                                anchors.verticalCenter: parent.verticalCenter
                                width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                                height: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                                name: placeRow.def.icon
                                fallbackName: placeRow.def.fallback
                                // Native theme colours (no tint).
                            }

                            Text {
                                x: placeIcon.x + placeIcon.width + 10
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - x - 12
                                text: placeRow.def.name
                                color: placeRow.current ? CelestinaTheme.accent
                                                        : CelestinaTheme.text
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.sidebarTextScale)
                                font.weight: placeRow.current ? CelestinaTheme.weightMedium
                                                              : CelestinaTheme.weightRegular
                                elide: Text.ElideRight
                            }

                            MouseArea {
                                id: placeMouse
                                anchors.fill: parent
                                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                drag.target: placeContent
                                drag.axis: Drag.YAxis
                                drag.smoothed: false
                                drag.threshold: 6
                                drag.minimumY: -placeRow.index * placesList.rowPitch
                                drag.maximumY: (placesList.count - 1 - placeRow.index)
                                               * placesList.rowPitch
                                preventStealing: true

                                onPositionChanged: {
                                    if (!drag.active)
                                        return
                                    placesList.dragIndex = placeRow.index
                                    placesList.dropIndex = Math.max(
                                        0, Math.min(placesList.count - 1,
                                                    placeRow.index + Math.round(
                                                        placeContent.y / placesList.rowPitch)))
                                }

                                onReleased: {
                                    if (placesList.dragIndex !== placeRow.index)
                                        return
                                    const from = placeRow.index
                                    const to = placesList.dropIndex
                                    placeRow.justDragged = true
                                    placeContent.y = 0
                                    placesList.moveDragged(from, to)
                                }

                                onCanceled: {
                                    placeContent.y = 0
                                    if (placesList.dragIndex === placeRow.index) {
                                        placesList.dragIndex = -1
                                        placesList.dropIndex = -1
                                    }
                                }

                                onClicked: function(mouse) {
                                    if (placeRow.justDragged) {
                                        placeRow.justDragged = false
                                        return
                                    }
                                    if (mouse.button === Qt.RightButton) {
                                        const point = placeRow.mapToItem(root.overlayParent,
                                                                         mouse.x, mouse.y)
                                        sidebarMenus.openPlace(placeRow.modelData,
                                                               placeRow.def.name,
                                                               placeRow.placePath,
                                                               point)
                                    } else if (mouse.button === Qt.MiddleButton) {
                                        if (placeRow.placePath.length > 0)
                                            root.hostWindow.openTab(placeRow.placePath, false)
                                    } else {
                                        placeRow.activate()
                                    }
                                }
                            }
                        }
                    }
                }

                // Bring hidden places back — the same escape hatch the devices
                // list offers, so hiding one is never a one-way door.
                Item {
                    width: placesColumn.width
                    readonly property int hidden: root.hostWindow.activeController
                                                  ? root.hostWindow.activeController.hiddenPlaceCount : 0
                    visible: hidden > 0 && !sidebar.placesCollapsed
                    height: visible ? root.hostWindow.sidebarRowHeight : 0

                    Text {
                        x: 12 + Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale) + 10
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Mostrar " + parent.hidden + " ocultos"
                        color: unhidePlacesMouse.containsMouse ? CelestinaTheme.accent
                                                               : CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontRowSecondary * root.hostWindow.sidebarTextScale)
                    }

                    MouseArea {
                        id: unhidePlacesMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: if (root.hostWindow.activeController)
                                       root.hostWindow.activeController.unhideAllPlaces()
                    }
                }

                // ── Removable volumes (UDisks2) ──────────────────────────
                Item {
                    width: placesColumn.width
                    readonly property var ac: root.hostWindow.activeController
                    readonly property int hiddenCount: ac ? ac.hiddenDeviceCount : 0
                    readonly property bool anyDevices:
                        ac && (ac.volumeNames.length > 0 || hiddenCount > 0)
                    height: anyDevices ? volumesHeaderRow.implicitHeight + 16 : 0
                    visible: anyDevices

                    SidebarChevron {

                        textScale: root.hostWindow.sidebarTextScale
                        x: -4
                        y: 13
                        collapsed: sidebar.devicesCollapsed
                        visible: parent.anyDevices
                    }

                    MouseArea {
                        width: parent.width
                        height: volumesHeaderRow.height + 12
                        y: 6
                        cursorShape: Qt.PointingHandCursor
                        onClicked: sidebar.devicesCollapsed = !sidebar.devicesCollapsed
                    }

                    CelestinaSectionLabel {
                        id: volumesHeaderRow
                        // placesColumn.x is 8, so x:8 here → the same absolute
                        // left edge as MARCADORES (x:16), aligning the headers.
                        x: 8
                        y: 12
                        text: "DISPOSITIVOS"
                        textScale: root.hostWindow.sidebarTextScale
                    }

                    // Un-hide affordance — reachable even when every device is
                    // hidden (the header still shows).
                    Text {
                        anchors.verticalCenter: volumesHeaderRow.verticalCenter
                        anchors.right: parent.right
                        anchors.rightMargin: 12
                        visible: parent.hiddenCount > 0
                        text: parent.hiddenCount + " ocultos"
                        color: unhideMouse.containsMouse ? CelestinaTheme.accent
                                                         : CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontMini * root.hostWindow.sidebarTextScale)

                        MouseArea {
                            id: unhideMouse
                            anchors.fill: parent
                            anchors.margins: -6
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: if (root.hostWindow.activeController)
                                           root.hostWindow.activeController.unhideAllDevices()
                        }
                    }
                }

                Repeater {
                    // Plegar vacía el modelo: las filas dejan de existir en vez
                    // de quedarse invisibles ocupando alto en la columna.
                    model: (root.hostWindow.activeController && !sidebar.devicesCollapsed)
                           ? root.hostWindow.activeController.volumeNames : []

                    delegate: Item {
                        id: volumeRow
                        required property int index
                        required property string modelData
                        readonly property string mountPoint:
                            (root.hostWindow.activeController
                             && index < root.hostWindow.activeController.volumeMounts.length)
                            ? root.hostWindow.activeController.volumeMounts[index] : ""
                        readonly property bool mounted: mountPoint.length > 0
                        readonly property bool current: mounted
                            && mountPoint === (root.hostWindow.activeController
                                               ? root.hostWindow.activeController.currentPath : "")

                        width: placesColumn.width
                        height: root.hostWindow.sidebarRowHeight
                        Accessible.role: Accessible.Button
                        Accessible.name: volumeRow.modelData
                                         + (volumeRow.mounted ? ", montado" : ", sin montar")

                        Rectangle {
                            anchors.fill: parent
                            anchors.leftMargin: 2
                            anchors.rightMargin: 2
                            radius: CelestinaTheme.radiusSm
                            color: volumeRow.current
                                   ? CelestinaTheme.badgeAccentFill
                                   : volumeMouse.containsMouse
                                     ? CelestinaTheme.surfaceHover : CelestinaTheme.clear
                        }

                        CelestinaIcon {
                            id: volumeIcon
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            height: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            name: "drive-removable-media"
                            fallbackName: "folder"
                            // Native theme colours (no tint).
                        }

                        Text {
                            x: volumeIcon.x + volumeIcon.width + 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: ejectButton.x - x - 6
                            text: volumeRow.modelData
                            color: volumeRow.current ? CelestinaTheme.accent
                                                     : CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.sidebarTextScale)
                            elide: Text.ElideRight
                        }

                        // Eject (unmount) when mounted; hidden otherwise.
                        CelestinaIcon {
                            id: ejectButton
                            z: 3   // above the full-row open handler below
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.right: parent.right
                            anchors.rightMargin: 10
                            width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            height: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            visible: volumeRow.mounted
                            name: "media-eject"
                            fallbackName: "media-eject"
                            tone: ejectMouse.containsMouse
                                  ? CelestinaIcon.Accent : CelestinaIcon.Secondary
                            opacity: ejectMouse.containsMouse
                                     ? 1 : CelestinaTheme.decorationOpacitySoft
                            Accessible.role: Accessible.Button
                            Accessible.name: "Expulsar " + volumeRow.modelData

                            MouseArea {
                                id: ejectMouse
                                anchors.fill: parent
                                anchors.margins: -4
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    if (root.hostWindow.activeController)
                                        root.hostWindow.activeController.unmountVolume(
                                            volumeRow.index)
                                }
                            }
                        }

                        MouseArea {
                            id: volumeMouse
                            anchors.fill: parent
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            // Left: open (mounting first if needed) — eject has its
                            // own zone. Right: hide this device.
                            onClicked: function(mouse) {
                                if (!root.hostWindow.activeController)
                                    return
                                if (mouse.button === Qt.RightButton) {
                                    const point = volumeRow.mapToItem(
                                                    root.overlayParent, mouse.x, mouse.y)
                                    sidebarMenus.openDevice(volumeRow.modelData, point)
                                } else {
                                    root.hostWindow.activeController.openVolume(volumeRow.index)
                                }
                            }
                        }
                    }
                }

                // ── Phone (Magnetita / org.celestina.Devices1) ───────────
                Item {
                    width: placesColumn.width
                    readonly property var ac: root.hostWindow.activeController
                    readonly property bool anyPhones: ac && ac.phoneNames.length > 0
                    height: anyPhones ? phoneHeader.implicitHeight + 16 : 0
                    visible: anyPhones

                    CelestinaSectionLabel {
                        id: phoneHeader
                        x: 8
                        y: 12
                        text: "MÓVIL"
                        textScale: root.hostWindow.sidebarTextScale
                    }
                }

                Repeater {
                    model: root.hostWindow.activeController
                           ? root.hostWindow.activeController.phoneNames : []

                    delegate: Item {
                        id: phoneRow
                        required property int index
                        required property string modelData
                        readonly property string mountPath:
                            (root.hostWindow.activeController
                             && index < root.hostWindow.activeController.phoneMounts.length)
                            ? root.hostWindow.activeController.phoneMounts[index] : ""
                        readonly property bool mounted: mountPath.length > 0
                        readonly property bool current: mounted
                            && mountPath === (root.hostWindow.activeController
                                              ? root.hostWindow.activeController.currentPath : "")

                        width: placesColumn.width
                        height: root.hostWindow.sidebarRowHeight
                        Accessible.role: Accessible.Button
                        Accessible.name: phoneRow.modelData
                                         + (phoneRow.mounted ? ", montado" : ", conectando")

                        Rectangle {
                            anchors.fill: parent
                            anchors.leftMargin: 2
                            anchors.rightMargin: 2
                            radius: CelestinaTheme.radiusSm
                            color: phoneRow.current
                                   ? CelestinaTheme.badgeAccentFill
                                   : (phoneRow.mounted && phoneMouse.containsMouse)
                                     ? CelestinaTheme.surfaceHover : CelestinaTheme.clear
                        }

                        CelestinaIcon {
                            id: phoneIcon
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            height: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            name: "phone"
                            fallbackName: "phone"
                            // Dim until the mount is ready.
                            opacity: phoneRow.mounted
                                     ? 1 : CelestinaTheme.disabledOpacity
                        }

                        Text {
                            x: phoneIcon.x + phoneIcon.width + 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - x - 10
                            text: phoneRow.mounted ? phoneRow.modelData
                                                   : phoneRow.modelData + " — conectando…"
                            color: phoneRow.current ? CelestinaTheme.accent
                                                    : phoneRow.mounted ? CelestinaTheme.text
                                                                       : CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.sidebarTextScale)
                            elide: Text.ElideRight
                        }

                        MouseArea {
                            id: phoneMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            // Only openable once mounted; connecting is not a target.
                            enabled: phoneRow.mounted
                            cursorShape: phoneRow.mounted ? Qt.PointingHandCursor
                                                          : Qt.ArrowCursor
                            onClicked: {
                                if (root.hostWindow.activeController && phoneRow.mounted)
                                    root.hostWindow.activeController.openPhone(phoneRow.index)
                            }
                        }
                    }
                }
            }


            SidebarSavedSections {
                id: savedSections
                x: 0
                y: placesColumn.y + placesColumn.height + 12
                width: parent.width
                hostWindow: root.hostWindow
                overlayParent: root.overlayParent

                onFavoriteMenuRequested: function(path, popupX, popupY) {
                    sidebarMenus.openFavorite(path, Qt.point(popupX, popupY))
                }

                onBookmarkMenuRequested: function(index, popupX, popupY) {
                    sidebarMenus.openBookmark(index, Qt.point(popupX, popupY))
                }
            }
        }

    }


    SidebarInfo {
        id: sidebarInfo
        x: sidebar.x
        width: sidebar.width
        height: Math.round(84 * root.hostWindow.sidebarTextScale)
        y: parent.height - height - 18
        visible: sidebar.visible
        hostWindow: root.hostWindow
    }

    SidebarContextMenus {
        id: sidebarMenus
        hostWindow: root.hostWindow
        overlayParent: root.overlayParent
        backdropSource: root.backdrop
        bookmarkCount: savedSections.bookmarkCount
        onEditBookmarkRequested: function(index) { savedSections.editBookmark(index) }
    }
}
