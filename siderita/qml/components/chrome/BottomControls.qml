import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.siderita 1.0

// ─── BottomControls ─────────────────────────────────────────────────────────
// La fila de controles del pie: ocultos, una pastilla de orden y una pastilla de
// modos de vista, seguidas del indicador de carga. Todo lo de fuera llega por
// propiedad; el chrome no alcanza ids de la vista que lo instancia.
// ──────────────────────────────────────────────────────────────────────────────
RowLayout {
    id: root

    property var controller
    property var panel
    property Item bottomView      // vista de fondo para el cristal de las pastillas
    property bool bottomFloating: false
    property Item overlayParent   // dónde se ancla el menú de orden
    property var sortMenu         // el menú de orden, para abrirlo
    property real textScale: 1.0

    spacing: 8

    HiddenTogglePill {
        Layout.preferredHeight: CelestinaTheme.controlHeightSm
        toggleChecked: root.controller.showHidden
        backdrop: root.bottomView
        floating: root.bottomFloating
        textScale: root.textScale
        onToggleRequested: root.controller.toggleHidden()
    }

    GlassPill {
        id: sortGroup
        Layout.preferredWidth: sortRow.implicitWidth + 8
        Layout.preferredHeight: CelestinaTheme.controlHeightSm
        backdrop: root.bottomView
        floating: root.bottomFloating
        fill: CelestinaTheme.controlFill

        Row {
            id: sortRow
            anchors.centerIn: parent
            spacing: 2

            CelestinaButton {
                id: sortButton

                readonly property var labels: [
                    "Nombre", "Tamaño", "Fecha", "Tipo"
                ]

                height: sortGroup.height - 4
                role: CelestinaButton.Ghost
                density: CelestinaButton.Compact
                text: labels[root.controller.sortField]
                Accessible.name: "Ordenar por " + text
                font.pixelSize: Math.round(CelestinaTheme.fontCaption
                                           * root.textScale)
                onClicked: {
                    // This control sits at the bottom, so its menu opens upward.
                    const menuHeight = root.sortMenu.height > 0
                                     ? root.sortMenu.height : 172
                    const point = sortButton.mapToItem(
                                    root.overlayParent, 0, -menuHeight - 6)
                    root.sortMenu.popup(root.overlayParent, point)
                }
            }

            CelestinaIconButton {
                id: sortDirectionButton
                width: sortGroup.height - 4
                height: width
                role: CelestinaButton.Ghost
                density: CelestinaButton.Compact
                iconName: root.controller.sortAscending
                          ? "view-sort-ascending"
                          : "view-sort-descending"
                fallbackIcon: root.controller.sortAscending
                              ? "view-sort-ascending"
                              : "view-sort-descending"
                Accessible.name: root.controller.sortAscending
                                 ? "Orden ascendente"
                                 : "Orden descendente"
                onClicked: root.controller.toggleSortDirection()
            }
        }
    }

    GlassPill {
        id: viewGroup
        Layout.preferredWidth: viewRow.implicitWidth + 8
        Layout.preferredHeight: CelestinaTheme.controlHeightSm
        backdrop: root.bottomView
        floating: root.bottomFloating
        fill: CelestinaTheme.controlFill

        Row {
            id: viewRow
            anchors.centerIn: parent
            spacing: 2

            Repeater {
                model: [
                    { mode: "grid", fallback: "view-grid", label: "Cuadrícula" },
                    { mode: "list", fallback: "view-list", label: "Lista" },
                    { mode: "details", fallback: "view-details", label: "Detalles" }
                ]

                delegate: CelestinaIconButton {
                    required property var modelData
                    width: viewGroup.height - 4
                    height: width
                    role: root.panel.viewMode === modelData.mode
                          ? CelestinaButton.Selected : CelestinaButton.Ghost
                    density: CelestinaButton.Compact
                    iconName: ""
                    fallbackIcon: modelData.fallback
                    Accessible.name: "Vista " + modelData.label
                    onClicked: {
                        root.panel.viewMode = modelData.mode
                        root.panel.persist()
                    }
                }
            }
        }
    }

    BusyIndicator {
        Layout.preferredWidth: 26
        Layout.preferredHeight: 26
        running: root.controller.loading
        visible: running
    }
}
