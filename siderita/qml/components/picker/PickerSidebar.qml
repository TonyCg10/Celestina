import QtQuick
import org.celestina.siderita 1.0

// ─── PickerSidebar ────────────────────────────────────────────────────────────
// A navigation-only counterpart to the main window's Sidebar, for the file
// portal's dialog. Same sections (lugares, dispositivos, favoritos,
// marcadores) and the same controller state, but no write verb reaches from
// here: no bookmark drag-to-create, no place/bookmark reordering, no context
// menu, no device eject. PickerWindow's own header comment explains why — a
// dialog another application is blocked waiting on must not be able to
// surprise it with a mutation the user didn't come here to make.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    required property var pickerController
    required property real sidebarIconScale
    required property real sidebarTextScale
    readonly property int rowHeight: Math.max(
        Math.round(CelestinaTheme.iconSm * sidebarIconScale) + 16,
        Math.round(CelestinaTheme.fontBody * sidebarTextScale) + 21)

    readonly property bool panelVisible: panel.visible
    readonly property real rightEdge: panel.x + panel.width

    // Adapts `controller` to the duck-typed `hostWindow` shape the shared
    // favourite-row and phone-section components expect, with `openTab` as a
    // no-op — a picker has no tabs, so a middle-click here does nothing
    // instead of erroring.
    QtObject {
        id: navHost
        readonly property var activeController: root.pickerController
        readonly property real sidebarIconScale: root.sidebarIconScale
        readonly property real sidebarTextScale: root.sidebarTextScale
        readonly property real sidebarRowHeight: root.rowHeight
        function openTab() {}
    }

    CelestinaSurface {
        id: panel
        x: 12
        y: 12
        // Narrower and available at a smaller width than the main-window
        // sidebar: places are central to this dialog and disappear only when
        // the user shrinks it beyond the room they need.
        width: 190
        height: parent.height - y - 12
        visible: parent.width >= 560
        role: CelestinaSurface.Panel

        property bool placesCollapsed: false
        property bool devicesCollapsed: false
        property bool favoritesCollapsed: false
        property bool bookmarksCollapsed: false

        Flickable {
            id: scroll
            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceSm
            clip: true
            contentWidth: width
            contentHeight: column.height
            boundsBehavior: Flickable.StopAtBounds

            Column {
                id: column
                width: parent.width
                spacing: 2

                SidebarSectionHeader {
                    id: placesHeader
                    width: column.width
                    title: "LUGARES"
                    textScale: root.sidebarTextScale
                    iconScale: root.sidebarIconScale
                    collapsed: panel.placesCollapsed
                    onActivated: panel.placesCollapsed = !panel.placesCollapsed
                }

                ListView {
                    id: placesList
                    width: column.width
                    height: panel.placesCollapsed ? 0 : count * (root.rowHeight + spacing)
                    visible: !panel.placesCollapsed
                    interactive: false
                    spacing: 2
                    model: root.pickerController.placeKeys

                    delegate: Item {
                        id: placeRow

                        required property string modelData

                        readonly property var def: CelestinaPlaceDefs.defs[modelData]
                                                   || ({ name: modelData, icon: "folder",
                                                         fallback: "folder" })
                        readonly property bool isTrash: modelData === "TRASH"
                        readonly property bool isRecent: modelData === "RECENT"
                        readonly property string placePath:
                                isTrash || isRecent ? "" : root.pickerController.placePath(modelData)
                        readonly property bool current: isTrash
                                ? root.pickerController.trashActive
                                : isRecent
                                ? root.pickerController.recentActive
                                : (placePath.length > 0
                                   && placePath === root.pickerController.currentPath)

                        width: placesList.width
                        height: root.rowHeight

                        Accessible.role: Accessible.Button
                        Accessible.name: def.name
                        Accessible.onPressAction: placeRow.activate()

                        function activate() {
                            if (isTrash)
                                root.pickerController.openTrash()
                            else if (isRecent)
                                root.pickerController.openRecent()
                            else if (placePath.length > 0)
                                root.pickerController.openLocation(placePath)
                        }

                        Rectangle {
                            anchors.fill: parent
                            anchors.leftMargin: 2
                            anchors.rightMargin: 2
                            radius: CelestinaTheme.radiusSm
                            color: placeRow.current
                                   ? CelestinaTheme.badgeAccentFill
                                   : placeMouse.containsMouse
                                     ? CelestinaTheme.surfaceHover
                                     : CelestinaTheme.clear

                            Behavior on color {
                                ColorAnimation { duration: CelestinaTheme.motionFast }
                            }
                        }

                        Rectangle {
                            visible: placeRow.current
                            x: 2
                            anchors.verticalCenter: parent.verticalCenter
                            width: CelestinaTheme.compSelectionIndicatorWidth
                            height: CelestinaTheme.compSelectionIndicatorHeight
                            radius: width / 2
                            color: CelestinaTheme.accent
                        }

                        CelestinaIcon {
                            id: placeIcon
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.round(CelestinaTheme.iconSm * root.sidebarIconScale)
                            height: width
                            name: placeRow.def.icon
                            fallbackName: placeRow.def.fallback
                            tone: placeRow.current ? CelestinaIcon.Accent : CelestinaIcon.Navigation
                        }

                        Text {
                            x: placeIcon.x + placeIcon.width + 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - x - 12
                            text: placeRow.def.name
                            color: placeRow.current ? CelestinaTheme.accentLink : CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.sidebarTextScale)
                            font.weight: placeRow.current ? CelestinaTheme.weightMedium
                                                          : CelestinaTheme.weightRegular
                            elide: Text.ElideRight
                        }

                        MouseArea {
                            id: placeMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: placeRow.activate()
                        }
                    }
                }

                SidebarSectionHeader {
                    id: devicesHeader
                    width: column.width
                    visible: root.pickerController.volumeNames.length > 0
                    height: visible ? implicitHeight : 0
                    title: "DISPOSITIVOS"
                    textScale: root.sidebarTextScale
                    iconScale: root.sidebarIconScale
                    collapsed: panel.devicesCollapsed
                    onActivated: panel.devicesCollapsed = !panel.devicesCollapsed
                }

                Repeater {
                    model: panel.devicesCollapsed ? [] : root.pickerController.volumeNames

                    delegate: Item {
                        id: volumeRow
                        required property int index
                        required property string modelData
                        readonly property string mountPoint:
                            index < root.pickerController.volumeMounts.length
                            ? root.pickerController.volumeMounts[index] : ""
                        readonly property bool current: mountPoint.length > 0
                            && mountPoint === root.pickerController.currentPath

                        width: column.width
                        height: root.rowHeight
                        Accessible.role: Accessible.Button
                        Accessible.name: volumeRow.modelData

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
                            width: Math.round(CelestinaTheme.iconSm * root.sidebarIconScale)
                            height: width
                            name: "drive-removable-media"
                            fallbackName: "folder"
                            tone: volumeRow.current ? CelestinaIcon.Accent : CelestinaIcon.Device
                        }

                        Text {
                            x: volumeIcon.x + volumeIcon.width + 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - x - 12
                            text: volumeRow.modelData
                            color: volumeRow.current ? CelestinaTheme.accentLink : CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.sidebarTextScale)
                            elide: Text.ElideRight
                        }

                        MouseArea {
                            id: volumeMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.pickerController.openVolume(volumeRow.index)
                        }
                    }
                }

                // Read-only like the rest of this sidebar: left-click opens,
                // middle-click is the `navHost.openTab` no-op, right-click's
                // `contextMenuRequested` is left unconnected below.
                SidebarPhoneSection {
                    width: column.width
                    hostWindow: navHost
                }

                SidebarSectionHeader {
                    id: favoritesHeader
                    width: column.width
                    visible: root.pickerController.favoriteEntries.length > 0
                    height: visible ? implicitHeight : 0
                    title: "FAVORITOS"
                    textScale: root.sidebarTextScale
                    iconScale: root.sidebarIconScale
                    collapsed: panel.favoritesCollapsed
                    onActivated: panel.favoritesCollapsed = !panel.favoritesCollapsed
                }

                ListView {
                    id: favoritesList
                    width: column.width
                    height: panel.favoritesCollapsed ? 0 : count * (root.rowHeight + spacing)
                    visible: !panel.favoritesCollapsed
                    interactive: false
                    spacing: 2
                    model: {
                        const rows = []
                        const entries = root.pickerController.favoriteEntries
                        for (let i = 0; i < entries.length; i++) {
                            const cut = entries[i].indexOf("\t")
                            if (cut <= 0)
                                continue
                            const path = entries[i].substring(0, cut)
                            const slash = path.lastIndexOf("/")
                            rows.push({
                                path: path,
                                kind: entries[i].substring(cut + 1),
                                name: slash >= 0 && slash < path.length - 1
                                      ? path.substring(slash + 1) : path
                            })
                        }
                        return rows
                    }

                    delegate: SidebarFavoriteRow {
                        required property var modelData
                        width: favoritesList.width
                        hostWindow: navHost
                        overlayParent: root
                        entry: modelData
                        // No context menu is wired up: a right-click here is a
                        // no-op, which is the point.
                    }
                }

                SidebarSectionHeader {
                    id: bookmarksHeader
                    width: column.width
                    visible: root.pickerController.bookmarkNames.length > 0
                    height: visible ? implicitHeight : 0
                    title: "MARCADORES"
                    textScale: root.sidebarTextScale
                    iconScale: root.sidebarIconScale
                    collapsed: panel.bookmarksCollapsed
                    onActivated: panel.bookmarksCollapsed = !panel.bookmarksCollapsed
                }

                ListView {
                    id: bookmarksList
                    width: column.width
                    height: panel.bookmarksCollapsed ? 0 : count * (root.rowHeight + spacing)
                    visible: !panel.bookmarksCollapsed
                    interactive: false
                    spacing: 2
                    model: root.pickerController.bookmarkNames

                    delegate: Item {
                        id: bookmarkRow
                        required property int index
                        required property string modelData
                        readonly property string bookmarkPath:
                            index < root.pickerController.bookmarkPaths.length
                            ? root.pickerController.bookmarkPaths[index] : ""
                        readonly property bool current: bookmarkPath.length > 0
                            && bookmarkPath === root.pickerController.currentPath

                        width: bookmarksList.width
                        height: root.rowHeight

                        Rectangle {
                            anchors.fill: parent
                            anchors.leftMargin: 2
                            anchors.rightMargin: 2
                            radius: CelestinaTheme.radiusSm
                            color: bookmarkRow.current
                                   ? CelestinaTheme.badgeAccentFill
                                   : bookmarkMouse.containsMouse
                                     ? CelestinaTheme.surfaceHover : CelestinaTheme.clear
                        }

                        CelestinaIcon {
                            id: bookmarkIcon
                            x: 12
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.round(CelestinaTheme.iconSm * root.sidebarIconScale)
                            height: width
                            name: "folder"
                            fallbackName: "folder"
                            tone: CelestinaIcon.Danger
                        }

                        Text {
                            x: bookmarkIcon.x + bookmarkIcon.width + 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - x - 12
                            text: bookmarkRow.modelData
                            color: bookmarkRow.current ? CelestinaTheme.accentLink : CelestinaTheme.text
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.sidebarTextScale)
                            elide: Text.ElideRight
                        }

                        MouseArea {
                            id: bookmarkMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: if (bookmarkRow.bookmarkPath.length > 0)
                                           root.pickerController.openLocation(bookmarkRow.bookmarkPath)
                        }
                    }
                }
            }
        }
    }
}
