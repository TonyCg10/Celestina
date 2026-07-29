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
        x: root.panel.floatingChromeX
        width: root.panel.floatingChromeWidth
        height: 40
        y: (root.tabBar.visible
            ? root.tabBar.y + root.tabBar.height
            : root.topBar.y + root.topBar.height)
           + CelestinaTheme.compFloatingGap
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
        x: root.panel.floatingChromeX
        width: root.panel.floatingChromeWidth
        height: 40
        y: (root.tabBar.visible
            ? root.tabBar.y + root.tabBar.height
            : root.topBar.y + root.topBar.height)
           + CelestinaTheme.compFloatingGap
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
        x: root.panel.floatingChromeX
        width: root.panel.floatingChromeWidth
        height: 40
        y: (root.tabBar.visible
            ? root.tabBar.y + root.tabBar.height
            : root.topBar.y + root.topBar.height)
           + CelestinaTheme.compFloatingGap
        visible: root.controller.trashActive
        controller: root.controller
        backdrop: root.topBar.activeView
        textScale: root.hostWindow.interfaceTextScale
    }

    DragScrollEdge {
        x: root.panel.contentFrameX
        y: root.panel.contentRowsY
        width: root.panel.contentFrameWidth
        view: root.topBar.activeView
        step: -18
        onExternalDrop: function(drop) {
            root.controller.dropUris(root.panel.urlsToPaths(drop.urls), "",
                                     root.panel.dropIsMove(drop))
            drop.accept()
        }
    }

    DragScrollEdge {
        x: root.panel.contentFrameX
        y: root.panel.contentFrameBottom - height
        width: root.panel.contentFrameWidth
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
        x: root.panel.floatingChromeX
        width: root.panel.floatingChromeWidth
        height: Math.round(CelestinaTheme.fontCaption
                           * root.hostWindow.contentTextScale) + 18
        y: root.panel.contentRowsY
        visible: root.fileList.detailsMode
        controller: root.controller
        view: root.fileList
        textScale: root.hostWindow.contentTextScale
    }
}
