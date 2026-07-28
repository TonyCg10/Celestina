import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

Item {
    id: root

    required property var pickerController
    required property Item contentSurface
    required property Item backdropView
    required property string requesterId
    required property bool saving
    required property bool gridScrolls
    required property var filterRows
    required property bool multiple
    required property int chosenCount
    required property string acceptText
    required property bool canAccept

    property alias nameText: nameField.text
    property alias filterIndex: filterCombo.currentIndex

    signal filterActivated(int index)
    signal acceptRequested
    signal cancelRequested

    Item {
        id: topPills
        x: root.contentSurface.x + 12
        y: root.contentSurface.y + 12
        width: root.contentSurface.width - 24
        height: root.saving ? 100 : 54

        GlassPill {
            id: pathPill
            backdrop: root.backdropView
            floating: root.gridScrolls
            width: parent.width - navigationRow.width - 12
            height: 54
            radius: CelestinaTheme.radiusSm

            Text {
                id: requesterLabel
                x: 14
                y: 9
                width: parent.width - 28
                text: root.requesterId.length > 0
                      ? "Para " + root.requesterId
                      : "Solicitado por otra aplicación"
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
                elide: Text.ElideRight
            }

            Text {
                x: 14
                y: requesterLabel.y + requesterLabel.height + 2
                width: parent.width - 28
                text: root.pickerController.currentPath
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                elide: Text.ElideMiddle
            }
        }

        Row {
            id: navigationRow
            anchors.right: parent.right
            y: 10
            spacing: 8

            FloatingButton {
                text: "Subir"
                helpText: "Subir"
                backdrop: root.backdropView
                floating: root.gridScrolls
                enabled: root.pickerController.canGoUp && !root.pickerController.loading
                onClicked: root.pickerController.goUp()
            }

            FloatingButton {
                text: "Inicio"
                helpText: "Inicio"
                backdrop: root.backdropView
                floating: root.gridScrolls
                onClicked: root.pickerController.goHome()
            }
        }

        CelestinaTextField {
            id: nameField
            visible: root.saving
            width: parent.width
            height: CelestinaTheme.controlHeight
            y: 62
            placeholderText: "Nombre del archivo"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            leftPadding: CelestinaTheme.compButtonPaddingHorizontal
            rightPadding: CelestinaTheme.compButtonPaddingHorizontal
            onAccepted: if (root.canAccept) root.acceptRequested()
            background: GlassPill {
                radius: CelestinaTheme.radiusSm
                backdrop: root.backdropView
                floating: root.gridScrolls
                fill: CelestinaTheme.inputFill
                border.width: CelestinaTheme.borderHairline
                border.color: nameField.activeFocus
                              ? CelestinaTheme.focusRing : CelestinaTheme.clear
            }
        }
    }

    Item {
        id: bottomPills
        x: root.contentSurface.x + 12
        width: root.contentSurface.width - 24
        height: 38
        y: root.contentSurface.y + root.contentSurface.height - height - 12

        ComboBox {
            id: filterCombo
            visible: root.filterRows.length > 1
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(300, parent.width * 0.4)
            height: 34
            model: root.filterRows
            textRole: "label"
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
            onActivated: root.filterActivated(currentIndex)

            contentItem: Text {
                leftPadding: CelestinaTheme.compTextFieldPaddingHorizontal
                rightPadding: filterCombo.indicator.width + 6
                text: filterCombo.displayText
                color: CelestinaTheme.text
                font: filterCombo.font
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
            }

            background: GlassPill {
                backdrop: root.backdropView
                floating: root.gridScrolls
                fill: filterCombo.hovered
                      ? CelestinaTheme.surfaceHover : CelestinaTheme.controlFill
            }
        }

        Text {
            anchors.left: filterCombo.visible ? filterCombo.right : parent.left
            anchors.leftMargin: filterCombo.visible ? 14 : 4
            anchors.verticalCenter: parent.verticalCenter
            text: root.multiple && root.chosenCount > 1
                  ? root.chosenCount + " seleccionados" : ""
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }

        Row {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10

            FloatingButton {
                text: "Cancelar"
                backdrop: root.backdropView
                floating: root.gridScrolls
                onClicked: root.cancelRequested()
            }

            FloatingButton {
                text: root.acceptText
                role: FloatingButton.Primary
                backdrop: root.backdropView
                floating: root.gridScrolls
                enabled: root.canAccept
                onClicked: root.acceptRequested()
            }
        }
    }
}
