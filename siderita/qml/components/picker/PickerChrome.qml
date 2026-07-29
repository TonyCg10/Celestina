import QtQuick
import org.celestina.siderita 1.0

Item {
    id: root

    required property var pickerController
    required property var hostWindow
    required property Item contentSurface
    required property Item backdropView
    required property bool saving
    required property bool gridScrolls
    required property var filterRows
    required property bool multiple
    required property int chosenCount
    required property string acceptText
    required property bool canAccept
    required property bool showHidden
    required property bool loading

    property alias nameText: nameField.text
    property int filterIndex: 0
    readonly property int scrollInset: gridScrolls
                                        ? CelestinaTheme.spaceMd : 0

    signal filterActivated(int index)
    signal toggleHiddenRequested
    signal acceptRequested
    signal cancelRequested
    signal viewFocusRequested

    function beginEditing() { topBar.beginEditing() }
    function focusSearch() { topBar.focusSearch() }

    Item {
        id: topPills
        x: root.contentSurface.x + 12
        y: root.contentSurface.y + 12
        width: root.contentSurface.width - 24 - root.scrollInset
        height: root.saving ? 100 : 54

        TopBar {
            id: topBar

            width: parent.width
            height: CelestinaTheme.controlHeightLg
            controller: root.pickerController
            activeView: root.backdropView
            hostWindow: root.hostWindow
            overlayParent: root
            pathMenu: null
            onViewFocusRequested: root.viewFocusRequested()
        }

        CelestinaTextField {
            id: nameField
            visible: root.saving
            width: parent.width
            height: CelestinaTheme.controlHeight
            y: topBar.height + CelestinaTheme.compFloatingGap
            placeholderText: "Nombre del archivo"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            leftPadding: CelestinaTheme.compButtonPaddingHorizontal
            rightPadding: CelestinaTheme.compButtonPaddingHorizontal
            onAccepted: if (root.canAccept) root.acceptRequested()
            background: GlassPill {
                inputShield: false
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
        width: root.contentSurface.width - 24 - root.scrollInset
        height: 38
        y: root.contentSurface.y + root.contentSurface.height - height - 12

        Row {
            id: leftActions

            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            spacing: CelestinaTheme.spaceSm

            HiddenTogglePill {
                toggleChecked: root.showHidden
                backdrop: root.backdropView
                floating: root.gridScrolls
                onToggleRequested: root.toggleHiddenRequested()
            }

            FloatingButton {
                id: filterButton

                visible: root.filterRows.length > 1
                width: Math.min(300, implicitWidth)
                text: root.filterIndex >= 0
                      && root.filterIndex < root.filterRows.length
                      ? root.filterRows[root.filterIndex].label
                      : "Todos los archivos"
                helpText: "Filtrar tipos de archivo"
                backdrop: root.backdropView
                floating: root.gridScrolls
                active: filterMenu.visible
                onClicked: {
                    const menuHeight = root.filterRows.length
                                     * CelestinaTheme.controlHeight
                                     + CelestinaTheme.compMenuPadding * 2
                    const point = filterButton.mapToItem(
                                    root, 0, -menuHeight - CelestinaTheme.spaceSm)
                    filterMenu.popup(root, point)
                }
            }
        }

        Text {
            anchors.left: leftActions.right
            anchors.leftMargin: CelestinaTheme.spaceMd
            anchors.right: actionRow.left
            anchors.rightMargin: CelestinaTheme.spaceMd
            anchors.verticalCenter: parent.verticalCenter
            text: root.loading ? "Cargando…"
                  : root.multiple && root.chosenCount > 1
                  ? root.chosenCount + " seleccionados" : ""
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideRight
        }

        Row {
            id: actionRow

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

    PickerFilterMenu {
        id: filterMenu

        backdropSource: root.backdropView
        rows: root.filterRows
        selectedIndex: root.filterIndex
        onFilterChosen: function(index) {
            root.filterIndex = index
            root.filterActivated(index)
            close()
        }
    }
}
