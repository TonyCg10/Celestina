import QtQuick
import org.celestina.siderita 1.0

// The whole lower floating layer shares one anchor inside the content frame:
// sort/view controls, transient status and size control cannot drift apart.
Item {
    id: root

    required property var controller
    required property var hostWindow
    required property Item panel
    required property Item contentSurface
    required property Item bottomView
    required property bool bottomFloating
    required property Item overlayParent
    required property var sortMenuItem

    Item {
        id: bottomBarItem
        x: root.contentSurface.x
        y: root.contentSurface.y + root.contentSurface.height
           - height - root.panel.floatingChromeInset
        width: root.contentSurface.width
        height: CelestinaTheme.controlHeightSm
    }

    BottomControls {
        id: bottomControlsItem
        x: bottomBarItem.x + root.panel.floatingChromeInset
        width: implicitWidth
        height: implicitHeight
        anchors.verticalCenter: bottomBarItem.verticalCenter
        controller: root.controller
        panel: root.panel
        bottomView: root.bottomView
        bottomFloating: root.bottomFloating
        overlayParent: root.overlayParent
        sortMenu: root.sortMenuItem
        textScale: root.hostWindow.interfaceTextScale
    }

    FolderBottomStatus {
        anchors.fill: parent
        controller: root.controller
        hostWindow: root.hostWindow
        panel: root.panel
        bottomControls: bottomControlsItem
        contentFrame: root.contentSurface
        bottomBar: bottomBarItem
        bottomView: root.bottomView
        bottomFloating: root.bottomFloating
    }
}
