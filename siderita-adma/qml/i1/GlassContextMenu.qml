import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

Menu {
    id: root

    required property Item backdropSource

    width: CelestinaTheme.menuWidth
    padding: CelestinaTheme.menuPadding
    margins: CelestinaTheme.menuMargins
    modal: false
    dim: false
    popupType: Popup.Item
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    transformOrigin: Item.TopLeft

    background: GlassSurface {
        id: glassBackground
        backdropSource: root.backdropSource
        captureEnabled: root.visible
    }

    enter: Transition {
        ParallelAnimation {
            NumberAnimation {
                property: "opacity"
                from: 0
                to: 1
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
            NumberAnimation {
                property: "scale"
                from: 0.96
                to: 1
                duration: CelestinaTheme.motionNormal
                easing.type: CelestinaTheme.easeEmphasized
                easing.overshoot: CelestinaTheme.overshoot
            }
        }
    }

    exit: Transition {
        ParallelAnimation {
            NumberAnimation {
                property: "opacity"
                from: 1
                to: 0
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeExit
            }
            NumberAnimation {
                property: "scale"
                from: 1
                to: 0.98
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeExit
            }
        }
    }

    onAboutToShow: Qt.callLater(function() {
        glassBackground.refreshBackdrop()
    })
}
