import QtQuick
import org.celestina.siderita 1.0

Item {
    id: root

    required property var controller
    required property var hostWindow
    required property Item panel
    required property Item bottomControls
    required property Item bottomView
    required property bool bottomFloating
    property int bottomBarHeight: 54

    readonly property real bottomBarY: height - bottomBarHeight

    Rectangle {
        id: errorBanner
        x: 16
        y: root.bottomBarY - 8 - height
        width: parent.width - 32
        height: errorText.implicitHeight + 22
        radius: CelestinaTheme.radiusSm
        visible: root.controller.errorText.length > 0
        color: CelestinaTheme.dangerFill
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.dangerBorder
        z: 3

        Text {
            id: errorText
            anchors.fill: parent
            anchors.margins: 11
            text: root.controller.errorText
            color: CelestinaTheme.dangerFillInk
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
            wrapMode: Text.Wrap
        }
    }

    Rectangle {
        id: operationErrorBanner
        x: 16
        y: (errorBanner.visible ? errorBanner.y : root.bottomBarY) - 8 - height
        width: parent.width - 32
        height: operationErrorText.implicitHeight + 22
        radius: CelestinaTheme.radiusSm
        visible: root.controller.opError.length > 0
        color: CelestinaTheme.dangerFill
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.dangerBorder
        z: 4

        Text {
            id: operationErrorText
            anchors.fill: parent
            anchors.margins: 11
            text: root.controller.opError
            color: CelestinaTheme.dangerFillInk
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
            wrapMode: Text.Wrap
        }
    }

    CelestinaSurface {
        id: operationProgress
        x: 16
        y: (operationErrorBanner.visible
            ? operationErrorBanner.y
            : (errorBanner.visible ? errorBanner.y : root.bottomBarY)) - 8 - height
        width: parent.width - 32
        height: 62
        visible: root.controller.opRunning
        role: CelestinaSurface.Tonal
        z: 5

        Text {
            id: progressTitle
            x: 12
            y: 9
            width: cancelButton.x - x - 12
            text: {
                let label = root.controller.opCurrent.length > 0
                            ? root.controller.opCurrent : "Preparando…"
                if (root.controller.opTotal > 1)
                    label += "  ·  " + (root.controller.opDone + 1)
                             + " de " + root.controller.opTotal
                return label
            }
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
            elide: Text.ElideMiddle
        }

        Text {
            x: 12
            anchors.top: progressTitle.bottom
            anchors.topMargin: 3
            width: cancelButton.x - x - 12
            text: root.controller.opDetail
            visible: root.controller.opDetail.length > 0
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontCaption
                                       * root.hostWindow.interfaceTextScale)
            elide: Text.ElideRight
        }

        Rectangle {
            x: 12
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 10
            width: cancelButton.x - x - 12
            height: CelestinaTheme.compLinearTrackHeight
            radius: height / 2
            color: CelestinaTheme.controlFill

            Rectangle {
                height: parent.height
                radius: height / 2
                color: CelestinaTheme.accent
                width: root.controller.opTotal > 0
                       ? parent.width * Math.min(1, root.controller.opDone
                                                 / root.controller.opTotal)
                       : 0
                Behavior on width {
                    NumberAnimation { duration: CelestinaTheme.motionFast }
                }
            }
        }

        CelestinaButton {
            id: cancelButton
            anchors.verticalCenter: parent.verticalCenter
            anchors.right: parent.right
            anchors.rightMargin: 12
            height: 28
            text: "Cancelar"
            Accessible.name: "Cancelar la operación"
            onClicked: root.controller.cancelOp()
        }
    }

    Text {
        id: statusLine
        x: root.bottomControls.x + root.bottomControls.width + 14
        y: root.bottomBarY + (root.bottomBarHeight - height) / 2
        width: Math.max(0, sizeButton.x - x - 12)
        text: root.controller.watchDegraded
              ? "⚠ Vigilancia perdida · instantánea"
              : root.controller.statusText
        color: root.controller.watchDegraded
               ? CelestinaTheme.dangerFillInk : CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: Math.round(CelestinaTheme.fontCaption
                                   * root.hostWindow.interfaceTextScale)
        elide: Text.ElideRight
    }

    FloatingButton {
        id: sizeButton
        height: CelestinaTheme.controlHeightSm
        anchors.right: parent.right
        anchors.rightMargin: 16
        y: root.bottomBarY + (root.bottomBarHeight - height) / 2
        text: "Tamaño"
        backdrop: root.bottomView
        floating: root.bottomFloating
        active: sizePopup.opened
        Accessible.name: "Ajustar tamaños"
        onClicked: sizePopup.opened ? sizePopup.close() : sizePopup.open()
        font.pixelSize: Math.round(CelestinaTheme.fontCaption
                                   * root.hostWindow.interfaceTextScale)

        SizePopup {
            id: sizePopup
            y: -height - 10
            x: sizeButton.width - width
            backdrop: root.panel
            hostWindow: root.hostWindow
        }
    }
}
