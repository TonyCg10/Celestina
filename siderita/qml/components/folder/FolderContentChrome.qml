import QtQuick
import org.celestina.siderita 1.0

Item {
    id: root

    required property var controller
    required property var hostWindow
    required property Item panel
    required property Item topBar
    required property Item tabBar
    required property Item fileList

    property alias searchBar: searchStatus
    property alias recentHeader: recentStatus
    property alias trashHeader: trashStatus
    property alias detailsHeader: detailsStatus

    SearchBar {
        id: searchStatus
        z: 10
        x: 12
        width: root.width - 24
        height: 40
        y: (root.tabBar.visible
            ? root.tabBar.y + root.tabBar.height
            : root.topBar.y + root.topBar.height) + 8
        visible: opacity > 0.01
        opacity: (root.controller.searchActive || root.controller.searchRunning) ? 1 : 0
        Behavior on opacity {
            NumberAnimation {
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }
        controller: root.controller
        backdrop: root.topBar.activeView
        textScale: root.hostWindow.interfaceTextScale
    }

    RecentHeader {
        id: recentStatus
        z: 10
        x: 12
        width: root.width - 24
        height: 40
        y: (root.tabBar.visible
            ? root.tabBar.y + root.tabBar.height
            : root.topBar.y + root.topBar.height) + 8
        visible: opacity > 0.01
        opacity: root.controller.recentActive ? 1 : 0
        Behavior on opacity {
            NumberAnimation {
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }
        controller: root.controller
        backdrop: root.topBar.activeView
        textScale: root.hostWindow.interfaceTextScale
    }

    TrashHeader {
        id: trashStatus
        z: 10
        x: 12
        width: root.width - 24
        height: 40
        y: (root.tabBar.visible
            ? root.tabBar.y + root.tabBar.height
            : root.topBar.y + root.topBar.height) + 8
        visible: root.controller.trashActive
        controller: root.controller
        backdrop: root.topBar.activeView
        textScale: root.hostWindow.interfaceTextScale
    }

    DragScrollEdge {
        x: 8
        y: 14
        width: parent.width - 16
        view: root.topBar.activeView
        step: -18
        onExternalDrop: function(drop) {
            root.controller.dropUris(root.panel.urlsToPaths(drop.urls), "",
                                     root.panel.dropIsMove(drop))
            drop.accept()
        }
    }

    DragScrollEdge {
        x: 8
        y: parent.height - 68 + 14 - height
        width: parent.width - 16
        view: root.topBar.activeView
        step: 18
        onExternalDrop: function(drop) {
            root.controller.dropUris(root.panel.urlsToPaths(drop.urls), "",
                                     root.panel.dropIsMove(drop))
            drop.accept()
        }
    }

    DetailsHeader {
        id: detailsStatus
        z: 10
        x: 8
        width: parent.width - 16
        height: Math.round(CelestinaTheme.fontCaption
                           * root.hostWindow.contentTextScale) + 18
        y: (root.tabBar.visible
            ? root.tabBar.y + root.tabBar.height
            : root.topBar.y + root.topBar.height) + 8
        visible: root.fileList.detailsMode
        controller: root.controller
        view: root.fileList
        textScale: root.hostWindow.contentTextScale
    }
}
