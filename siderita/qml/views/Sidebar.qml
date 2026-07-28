import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
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

        // Cada zona se pliega por su cabecera. Vive en la sesión y no en
        // disco a propósito: plegar es un gesto de "ahora estorba", no una
        // preferencia — al abrir Siderita el panel enseña lo que hay.
        property bool placesCollapsed: false
        property bool devicesCollapsed: false
        property bool favoritesCollapsed: false
        property bool bookmarksCollapsed: false

        // ── Favourites ───────────────────────────────────────────────
        // Where the stars land. A starred folder opens; a starred file
        // reveals itself in its folder rather than launching an app from a
        // single click. The section disappears entirely when nothing is
        // starred, so the sidebar costs nothing until it is used.
        readonly property var favoriteRows: {
            var rows = []
            var ac = root.hostWindow.activeController
            var entries = ac ? ac.favoriteEntries : []
            for (var i = 0; i < entries.length; i++) {
                var cut = entries[i].indexOf("\t")
                if (cut <= 0)
                    continue
                var path = entries[i].substring(0, cut)
                var slash = path.lastIndexOf("/")
                rows.push({
                    path: path,
                    kind: entries[i].substring(cut + 1),
                    name: slash >= 0 && slash < path.length - 1
                          ? path.substring(slash + 1) : path
                })
            }
            return rows
        }

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
            contentHeight: bookmarksList.y + bookmarksList.height + 14
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

                    Text {
                        x: 8
                        y: 8
                        text: "LUGARES"
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontMini * root.hostWindow.sidebarTextScale)
                        font.letterSpacing: 1.4
                        font.weight: CelestinaTheme.weightDemiBold
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
                            height: 2
                            radius: 1
                            y: placesList.dropIndex > placesList.dragIndex
                               ? parent.height - height : 0
                            color: CelestinaTheme.accent
                        }

                        Item {
                            id: placeContent
                            width: placeRow.width
                            height: placeRow.height
                            opacity: placeRow.dragging ? 0.9 : 1

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
                                           : "transparent"

                                Behavior on color {
                                    ColorAnimation { duration: CelestinaTheme.motionFast }
                                }
                            }

                            IconImage {
                                id: placeIcon
                                x: 12
                                anchors.verticalCenter: parent.verticalCenter
                                width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                                height: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                                name: placeRow.def.icon
                                source: CelestinaTheme.fallbackIcon(placeRow.def.fallback)
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
                                        placeMenu.targetKey = placeRow.modelData
                                        placeMenu.targetName = placeRow.def.name
                                        placeMenu.targetPath = placeRow.placePath
                                        placeMenu.popup(root.overlayParent, point)
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

                    Text {
                        id: volumesHeaderRow
                        // placesColumn.x is 8, so x:8 here → the same absolute
                        // left edge as MARCADORES (x:16), aligning the headers.
                        x: 8
                        y: 12
                        text: "DISPOSITIVOS"
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontMini * root.hostWindow.sidebarTextScale)
                        font.letterSpacing: 1.4
                        font.weight: CelestinaTheme.weightDemiBold
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
                                     ? CelestinaTheme.surfaceHover : "transparent"
                        }

                        IconImage {
                            id: volumeIcon
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            height: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            name: "drive-removable-media"
                            source: CelestinaTheme.fallbackIcon("folder")
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
                        IconImage {
                            id: ejectButton
                            z: 3   // above the full-row open handler below
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.right: parent.right
                            anchors.rightMargin: 10
                            width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            height: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            visible: volumeRow.mounted
                            name: "media-eject"
                            source: CelestinaTheme.fallbackIcon("media-eject")
                            color: ejectMouse.containsMouse ? CelestinaTheme.accent
                                                            : CelestinaTheme.textMuted
                            opacity: ejectMouse.containsMouse ? 1.0 : 0.7
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
                                    deviceMenu.deviceName = volumeRow.modelData
                                    const point = volumeRow.mapToItem(
                                                    root.overlayParent, mouse.x, mouse.y)
                                    deviceMenu.popup(root.overlayParent, point)
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

                    Text {
                        id: phoneHeader
                        x: 8
                        y: 12
                        text: "MÓVIL"
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontMini * root.hostWindow.sidebarTextScale)
                        font.letterSpacing: 1.4
                        font.weight: CelestinaTheme.weightDemiBold
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
                                     ? CelestinaTheme.surfaceHover : "transparent"
                        }

                        IconImage {
                            id: phoneIcon
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            height: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            name: "phone"
                            source: CelestinaTheme.fallbackIcon("phone")
                            // Dim until the mount is ready.
                            opacity: phoneRow.mounted ? 1.0 : 0.5
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


            Text {
                id: favoritesLabel
                x: 16
                y: placesColumn.y + placesColumn.height + 12
                visible: sidebar.favoriteRows.length > 0
                text: "FAVORITOS"

                SidebarChevron {

                    textScale: root.hostWindow.sidebarTextScale
                    x: -12
                    anchors.verticalCenter: parent.verticalCenter
                    collapsed: sidebar.favoritesCollapsed
                }

                MouseArea {
                    anchors.fill: parent
                    anchors.margins: -6
                    anchors.leftMargin: -16
                    cursorShape: Qt.PointingHandCursor
                    onClicked: sidebar.favoritesCollapsed = !sidebar.favoritesCollapsed
                }

                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: Math.round(CelestinaTheme.fontMini * root.hostWindow.sidebarTextScale)
                font.letterSpacing: 1.4
                font.weight: CelestinaTheme.weightDemiBold
            }

            ListView {
                id: favoritesList
                x: 8
                y: favoritesLabel.y + (favoritesLabel.visible ? 20 : 0)
                width: parent.width - 16
                // Sized to its content and never scrollable: a section that
                // scrolls inside a sidebar that also scrolls is two scrollbars
                // arguing. The bookmarks below take whatever is left.
                height: sidebar.favoritesCollapsed
                        ? 0 : count * (root.hostWindow.sidebarRowHeight + spacing)
                interactive: false
                visible: count > 0 && !sidebar.favoritesCollapsed
                clip: true
                model: sidebar.favoriteRows
                spacing: 2
                boundsBehavior: Flickable.StopAtBounds

                delegate: Item {
                    id: favRow

                    required property var modelData
                    readonly property bool missing: modelData.kind === "missing"
                    readonly property bool current: !missing
                            && modelData.path === (root.hostWindow.activeController
                                                   ? root.hostWindow.activeController.currentPath : "")

                    width: favoritesList.width
                    height: root.hostWindow.sidebarRowHeight

                    Rectangle {
                        anchors.fill: parent
                        anchors.leftMargin: 2
                        anchors.rightMargin: 2
                        radius: CelestinaTheme.radiusSm
                        color: favRow.current
                               ? CelestinaTheme.badgeAccentFill
                               : favMouse.containsMouse
                                 ? CelestinaTheme.surfaceHover
                                 : "transparent"

                        Behavior on color {
                            ColorAnimation { duration: CelestinaTheme.motionFast }
                        }
                    }

                    IconImage {
                        id: favIcon
                        x: 12
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                        height: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                        opacity: favRow.missing ? 0.45 : 1
                        name: favRow.modelData.kind === "directory"
                              ? "folder" : "text-x-generic"
                        source: CelestinaTheme.fallbackIcon(
                                    favRow.modelData.kind === "directory" ? "folder" : "file")
                    }

                    Text {
                        x: favIcon.x + favIcon.width + 10
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - x - 12
                        text: favRow.modelData.name
                        color: favRow.missing ? CelestinaTheme.textMuted
                               : favRow.current ? CelestinaTheme.accent
                               : CelestinaTheme.text
                        // A favourite whose target is gone is struck through, not
                        // hidden: it is still a star the user set, and only they
                        // should decide to drop it.
                        font.strikeout: favRow.missing
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.sidebarTextScale)
                        font.weight: favRow.current ? CelestinaTheme.weightMedium
                                                    : CelestinaTheme.weightRegular
                        elide: Text.ElideMiddle
                    }

                    MouseArea {
                        id: favMouse
                        anchors.fill: parent
                        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        ToolTip.visible: containsMouse
                        ToolTip.delay: 600
                        ToolTip.text: favRow.modelData.path
                        onClicked: function(mouse) {
                            const ac = root.hostWindow.activeController
                            if (!ac || favRow.missing)
                                return
                            if (mouse.button === Qt.RightButton) {
                                const point = favRow.mapToItem(root.overlayParent,
                                                               mouse.x, mouse.y)
                                favMenu.targetPath = favRow.modelData.path
                                favMenu.popup(root.overlayParent, point)
                            } else if (favRow.modelData.kind === "directory") {
                                if (mouse.button === Qt.MiddleButton)
                                    root.hostWindow.openTab(favRow.modelData.path, false)
                                else
                                    ac.openLocation(favRow.modelData.path)
                            } else {
                                ac.revealPath(favRow.modelData.path)
                            }
                        }
                    }
                }
            }

            Text {
                id: bookmarksLabel
                x: 16
                // Se mide contra la *cabecera* de favoritos, no contra su
                // lista: plegada la lista mide 0 pero la cabecera sigue ahí,
                // y contra la lista los dos títulos se pisaban.
                y: favoritesLabel.visible
                   ? favoritesList.y + favoritesList.height + 12
                   : favoritesLabel.y
                text: "MARCADORES"

                SidebarChevron {

                    textScale: root.hostWindow.sidebarTextScale
                    x: -12
                    anchors.verticalCenter: parent.verticalCenter
                    collapsed: sidebar.bookmarksCollapsed
                }

                MouseArea {
                    anchors.fill: parent
                    anchors.margins: -6
                    anchors.leftMargin: -16
                    cursorShape: Qt.PointingHandCursor
                    onClicked: sidebar.bookmarksCollapsed = !sidebar.bookmarksCollapsed
                }

                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: Math.round(CelestinaTheme.fontMini * root.hostWindow.sidebarTextScale)
                font.letterSpacing: 1.4
                font.weight: CelestinaTheme.weightDemiBold
            }

            ListView {
                id: bookmarksList
                x: 8
                y: bookmarksLabel.y + 20
                width: parent.width - 16
                // Mide lo que ocupa: quien desplaza es el panel entero.
                height: sidebar.bookmarksCollapsed
                        ? 0 : count * (root.hostWindow.sidebarRowHeight + spacing)
                visible: !sidebar.bookmarksCollapsed
                clip: true
                model: root.hostWindow.activeController ? root.hostWindow.activeController.bookmarkNames : []
                spacing: 2
                boundsBehavior: Flickable.StopAtBounds

                property int editIndex: -1

                // ── Reordering ───────────────────────────────────────────
                // The bookmarks are the user's own list, so their order is
                // theirs too: drag a row to move it. `dragIndex` is the row
                // being carried, `dropIndex` where it would land; the row
                // lifts and every other row shows the landing line.
                property int dragIndex: -1
                property int dropIndex: -1
                readonly property int rowPitch: root.hostWindow.sidebarRowHeight + spacing

                function moveDragged(from, to) {
                    dragIndex = -1
                    dropIndex = -1
                    // Last: the move republishes bookmarkNames, which resets
                    // this view and destroys the delegate that called us.
                    if (to >= 0 && to !== from && root.hostWindow.activeController)
                        root.hostWindow.activeController.moveBookmark(from, to)
                }

                delegate: Item {
                    id: bmRow

                    required property int index
                    required property string modelData

                    readonly property string path: (root.hostWindow.activeController
                            && index >= 0
                            && index < root.hostWindow.activeController.bookmarkPaths.length)
                            ? root.hostWindow.activeController.bookmarkPaths[index] : ""
                    readonly property bool current: path.length > 0
                            && path === (root.hostWindow.activeController
                                         ? root.hostWindow.activeController.currentPath : "")
                    readonly property bool editing: bookmarksList.editIndex === index
                    readonly property bool dragging: bookmarksList.dragIndex === index
                    // Set on release so the click that ends a drag doesn't also
                    // navigate; cleared by the click itself.
                    property bool justDragged: false

                    width: bookmarksList.width
                    height: root.hostWindow.sidebarRowHeight
                    // On the delegate, not on its content: z inside a row would
                    // not lift it above the neighbouring rows it overlaps.
                    z: dragging ? 2 : 0

                    // Where the carried row would land, drawn on the row it
                    // would land against — above it when moving up, below when
                    // moving down.
                    Rectangle {
                        z: 3
                        visible: bookmarksList.dragIndex >= 0
                                 && bookmarksList.dragIndex !== bmRow.index
                                 && bookmarksList.dropIndex === bmRow.index
                        x: 2
                        width: parent.width - 4
                        height: 2
                        radius: 1
                        y: bookmarksList.dropIndex > bookmarksList.dragIndex
                           ? parent.height - height : 0
                        color: CelestinaTheme.accent
                    }

                    // Everything visible lives in here so a drag can carry the
                    // row without fighting the view, which owns the delegate's
                    // own position.
                    Item {
                        id: bmContent
                        width: bmRow.width
                        height: bmRow.height
                        opacity: bmRow.dragging ? 0.9 : 1

                        Behavior on y {
                            enabled: !bmMouse.drag.active
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
                            color: bmRow.dragging
                                   ? CelestinaTheme.surfaceStrong
                                   : bmRow.current
                                     ? CelestinaTheme.badgeAccentFill
                                     : bmMouse.containsMouse
                                       ? CelestinaTheme.surfaceHover
                                       : "transparent"

                            Behavior on color {
                                ColorAnimation { duration: CelestinaTheme.motionFast }
                            }
                        }

                        IconImage {
                            id: bmIcon
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            height: Math.round(CelestinaTheme.iconSm * root.hostWindow.sidebarIconScale)
                            name: "folder"
                            source: CelestinaTheme.fallbackIcon("folder")
                            // Native theme colours (no tint).
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
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.hostWindow.sidebarTextScale)
                            font.weight: bmRow.current ? CelestinaTheme.weightMedium
                                                       : CelestinaTheme.weightRegular
                            elide: Text.ElideRight
                        }

                        CelestinaTextField {
                            id: bmField
                            visible: bmRow.editing
                            x: bmIcon.x + bmIcon.width + 6
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - x - 8
                            height: 26
                            radius: CelestinaTheme.radiusSm
                            text: bmRow.modelData
                            font.pixelSize: CelestinaTheme.fontRowSecondary
                            leftPadding: 8
                            rightPadding: 8
                            onVisibleChanged: if (visible) { forceActiveFocus(); selectAll() }
                            // Leave edit mode *before* renaming: the rename republishes
                            // bookmarkNames, which resets this ListView and destroys
                            // this very delegate — anything touched afterwards (the
                            // row's index, the list's id) is already gone.
                            onAccepted: {
                                const index = bmRow.index
                                const value = text
                                bookmarksList.editIndex = -1
                                if (root.hostWindow.activeController)
                                    root.hostWindow.activeController.renameBookmark(index, value)
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
                            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            // Carry the row itself, bounded to the list so it can
                            // never be dragged out of its own section. The list is
                            // a Flickable, so hold the grab once the drag starts or
                            // it would scroll instead of reorder.
                            drag.target: bmRow.editing ? null : bmContent
                            drag.axis: Drag.YAxis
                            drag.smoothed: false
                            drag.threshold: 6
                            drag.minimumY: -bmRow.index * bookmarksList.rowPitch
                            drag.maximumY: (bookmarksList.count - 1 - bmRow.index)
                                           * bookmarksList.rowPitch
                            preventStealing: true

                            onPositionChanged: {
                                if (!drag.active)
                                    return
                                bookmarksList.dragIndex = bmRow.index
                                bookmarksList.dropIndex = Math.max(
                                    0, Math.min(bookmarksList.count - 1,
                                                bmRow.index + Math.round(
                                                    bmContent.y / bookmarksList.rowPitch)))
                            }

                            onReleased: {
                                if (bookmarksList.dragIndex !== bmRow.index)
                                    return
                                const from = bmRow.index
                                const to = bookmarksList.dropIndex
                                bmRow.justDragged = true
                                bmContent.y = 0
                                bookmarksList.moveDragged(from, to)
                            }

                            onCanceled: {
                                bmContent.y = 0
                                if (bookmarksList.dragIndex === bmRow.index) {
                                    bookmarksList.dragIndex = -1
                                    bookmarksList.dropIndex = -1
                                }
                            }

                            onClicked: function(mouse) {
                                // The release that ends a reorder is not a click.
                                if (bmRow.justDragged) {
                                    bmRow.justDragged = false
                                    return
                                }
                                if (mouse.button === Qt.MiddleButton) {
                                    root.hostWindow.openTab(bmRow.path, false)
                                } else if (mouse.button === Qt.RightButton) {
                                    const point = bmRow.mapToItem(root.overlayParent,
                                                                  mouse.x, mouse.y)
                                    bmMenu.targetIndex = bmRow.index
                                    bmMenu.popup(root.overlayParent, point)
                                } else if (root.hostWindow.activeController) {
                                    root.hostWindow.activeController.openLocation(bmRow.path)
                                }
                            }
                            onDoubleClicked: bookmarksList.editIndex = bmRow.index
                        }
                    }
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

    GlassContextMenu {
        id: bmMenu
        backdropSource: root.backdrop

        property int targetIndex: -1

        GlassMenuItem {
            text: "Renombrar"
            onTriggered: bookmarksList.editIndex = bmMenu.targetIndex
        }

        // The same reorder the drag does, reachable from the keyboard and a
        // screen reader — a drag is not an accessible way to be the only one.
        GlassMenuItem {
            text: "Subir"
            enabled: bmMenu.targetIndex > 0
            icon.name: "go-up"
            icon.source: CelestinaTheme.fallbackIcon("go-up")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.moveBookmark(bmMenu.targetIndex,
                                                         bmMenu.targetIndex - 1)
            }
        }

        GlassMenuItem {
            text: "Bajar"
            enabled: bmMenu.targetIndex >= 0
                     && bmMenu.targetIndex < bookmarksList.count - 1
            icon.name: "go-down"
            icon.source: CelestinaTheme.fallbackIcon("go-up")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.moveBookmark(bmMenu.targetIndex,
                                                         bmMenu.targetIndex + 1)
            }
        }

        GlassMenuItem {
            text: "Quitar de marcadores"
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.removeBookmark(bmMenu.targetIndex)
            }
        }
    }

    // Right-click menu for a sidebar place.
    GlassContextMenu {
        id: placeMenu
        backdropSource: root.backdrop

        property string targetKey: ""
        property string targetName: ""
        property string targetPath: ""

        GlassMenuItem {
            text: "Abrir en pestaña nueva"
            visible: placeMenu.targetPath.length > 0
            height: visible ? implicitHeight : 0
            icon.name: "tab-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: root.hostWindow.openTab(placeMenu.targetPath, true)
        }

        GlassMenuItem {
            text: "Ocultar «" + placeMenu.targetName + "»"
            icon.name: "list-remove"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.hidePlace(placeMenu.targetKey)
            }
        }

        GlassMenuItem {
            text: "Mostrar lugares ocultos"
            visible: root.hostWindow.activeController
                     && root.hostWindow.activeController.hiddenPlaceCount > 0
            height: visible ? implicitHeight : 0
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.unhideAllPlaces()
            }
        }
    }

    // Right-click menu for a row in the "Favoritos" list.
    GlassContextMenu {
        id: favMenu
        backdropSource: root.backdrop

        property string targetPath: ""

        GlassMenuItem {
            text: "Abrir en pestaña nueva"
            icon.name: "tab-new"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: root.hostWindow.openTab(favMenu.targetPath, true)
        }

        GlassMenuItem {
            text: "Mostrar en su carpeta"
            icon.name: "folder-open"
            icon.source: CelestinaTheme.fallbackIcon("folder")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.revealPath(favMenu.targetPath)
            }
        }

        GlassMenuItem {
            text: "Quitar de favoritos"
            icon.source: CelestinaTheme.fallbackIcon("star")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.toggleFavorite(favMenu.targetPath)
            }
        }
    }

    // Right-click menu for a device in the "Dispositivos" list.
    GlassContextMenu {
        id: deviceMenu
        backdropSource: root.backdrop

        property string deviceName: ""

        GlassMenuItem {
            text: "Ocultar dispositivo"
            icon.name: "list-remove"
            icon.source: CelestinaTheme.fallbackIcon("file")
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.hideDevice(deviceMenu.deviceName)
            }
        }

        GlassMenuItem {
            text: "Mostrar dispositivos ocultos"
            visible: root.hostWindow.activeController
                     && root.hostWindow.activeController.hiddenDeviceCount > 0
            height: visible ? implicitHeight : 0
            onTriggered: {
                if (root.hostWindow.activeController)
                    root.hostWindow.activeController.unhideAllDevices()
            }
        }
    }
}
