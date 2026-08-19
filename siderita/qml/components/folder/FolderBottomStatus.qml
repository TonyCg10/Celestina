import QtQuick
import org.celestina.siderita 1.0

Item {
    id: root

    required property var controller
    required property var hostWindow
    required property Item panel
    required property Item bottomControls
    required property Item contentFrame
    required property Item bottomBar
    required property Item bottomView
    required property bool bottomFloating

    Rectangle {
        id: errorBanner
        x: root.contentFrame.x + root.panel.floatingChromeInset
        y: root.bottomBar.y - CelestinaTheme.compFloatingGap - height
        width: root.contentFrame.width
               - 2 * root.panel.floatingChromeInset
        height: errorText.implicitHeight + 22
        radius: CelestinaTheme.radiusSm
        visible: root.controller.errorText.length > 0
        color: CelestinaTheme.dangerFill
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.dangerBorder
        z: 3

        // The banner covers several rows for as long as it lasts. With no input
        // floor, clicking its text selected, opened the menu of, or dragged the
        // file hidden behind it.
        CelestinaInputShield { }

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
        x: root.contentFrame.x + root.panel.floatingChromeInset
        y: (errorBanner.visible ? errorBanner.y : root.bottomBar.y)
           - CelestinaTheme.compFloatingGap - height
        width: root.contentFrame.width
               - 2 * root.panel.floatingChromeInset
        height: operationErrorText.implicitHeight + 22
        radius: CelestinaTheme.radiusSm
        visible: root.controller.opError.length > 0
        color: CelestinaTheme.dangerFill
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.dangerBorder
        z: 4

        CelestinaInputShield { }

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

    // The running operations, as rings over the content rather than a bar
    // spanning the window for an hour. It rests on the right, above the bottom
    // bar, and grows leftwards as more jobs appear.
    OperationsDock {
        id: operationsDock
        controller: root.controller
        backdrop: root.bottomView
        // Always floating: the dock sits over the content by definition, and
        // switching the glass off at the end of the list left it flat and
        // opaque.
        floating: true
        x: root.contentFrame.x + root.contentFrame.width
           - root.panel.floatingChromeInset - width
        y: (operationErrorBanner.visible
            ? operationErrorBanner.y
            : (errorBanner.visible ? errorBanner.y : root.bottomBar.y))
           - CelestinaTheme.compFloatingGap - height
        z: 5
    }

    GlassPill {
        id: statusPill
        x: root.bottomControls.x + root.bottomControls.implicitWidth
           + CelestinaTheme.spaceMd
        height: CelestinaTheme.controlHeightSm
        y: root.bottomBar.y + (root.bottomBar.height - height) / 2
        width: Math.max(0, sizeButton.x - x - 12)
        visible: width > 80 && statusLine.text.length > 0
        backdrop: root.bottomView
        // Always floating: the dock sits over the content by definition, and
        // switching the glass off at the end of the list left it flat and
        // opaque.
        floating: true
        fill: CelestinaTheme.controlFill

        CelestinaIcon {
            id: statusWarning
            anchors.left: parent.left
            anchors.leftMargin: CelestinaTheme.spaceMd
            anchors.verticalCenter: parent.verticalCenter
            width: Math.round(CelestinaTheme.iconSm
                              * root.hostWindow.interfaceIconScale)
            height: width
            visible: root.controller.watchDegraded
            name: "circle-alert"
            fallbackName: "circle-alert"
            tone: CelestinaIcon.Danger
        }

        Text {
            id: statusLine
            anchors.fill: parent
            anchors.leftMargin: statusWarning.visible
                                ? statusWarning.x + statusWarning.width
                                  + CelestinaTheme.spaceSm
                                : CelestinaTheme.spaceMd
            anchors.rightMargin: CelestinaTheme.spaceMd
            text: root.controller.watchDegraded
                  ? "Vigilancia perdida · instantánea"
                  : root.controller.statusText
            color: root.controller.watchDegraded
                   ? CelestinaTheme.dangerFillInk : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontCaption
                                       * root.hostWindow.interfaceTextScale)
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }
    }

    FloatingButton {
        id: sizeButton
        height: CelestinaTheme.controlHeightSm
        x: root.contentFrame.x + root.contentFrame.width
           - width - root.panel.floatingChromeInset
        y: root.bottomBar.y + (root.bottomBar.height - height) / 2
        text: "Tamaño"
        backdrop: root.bottomView
        // Always floating: the dock sits over the content by definition, and
        // switching the glass off at the end of the list left it flat and
        // opaque.
        floating: true
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
