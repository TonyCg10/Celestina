import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.siderita 1.0

// ─── BottomControls ─────────────────────────────────────────────────────────
// La fila de controles del pie: ocultos, el botón de orden (abre su menú), el
// sentido del orden, los tres modos de vista (lista/cuadrícula/detalles) y el
// indicador de carga. Pastillas sueltas sobre el contenido, con cristal cuando
// aún queda lista debajo. Todo lo de fuera llega por propiedad — el controlador,
// el panel (modo de vista), el fondo/flotación del pie, el menú de orden y dónde
// abrirlo, y la escala de texto — así el componente no alcanza ids externos.
// ──────────────────────────────────────────────────────────────────────────────
RowLayout {
    id: root

    property var controller
    property var panel            // mainPanel: modo de vista + persist
    property Item bottomView      // vista de fondo para el cristal de las pastillas
    property bool bottomFloating: false
    property Item overlayParent   // dónde se ancla el menú de orden
    property var sortMenu         // el menú de orden, para abrirlo
    property real textScale: 1.0

    spacing: 10

    GlassPill {
        id: hiddenToggle
        Layout.preferredWidth: hiddenLabel.implicitWidth + 22
        Layout.preferredHeight: 30
        backdrop: root.bottomView
        floating: root.bottomFloating
        fill: root.controller.showHidden
              ? CelestinaTheme.badgeAccentFill
              : hiddenMouse.containsMouse
                ? CelestinaTheme.surfaceHover
                : CelestinaTheme.controlFill

        Accessible.role: Accessible.Button
        Accessible.name: "Mostrar u ocultar elementos ocultos"

        Text {
            id: hiddenLabel
            anchors.centerIn: parent
            text: "Ocultos"
            color: root.controller.showHidden
                   ? CelestinaTheme.accent
                   : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontMini * root.textScale)
            font.weight: CelestinaTheme.weightMedium
        }

        MouseArea {
            id: hiddenMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.controller.toggleHidden()
        }
    }

    Button {
        id: sortButton

        readonly property var labels: [
            "Nombre", "Tamaño", "Fecha", "Tipo"
        ]

        Layout.preferredHeight: 34
        leftPadding: 16
        rightPadding: 16
        text: labels[root.controller.sortField]
        Accessible.name: "Ordenar por " + text
        onClicked: {
            // Button is at the bottom now — open the menu upward.
            // sortMenu.height can be 0 before the first open; fall back
            // to an estimate for the four sort options.
            const menuHeight = root.sortMenu.height > 0 ? root.sortMenu.height : 172
            const point = sortButton.mapToItem(
                            root.overlayParent, 0, -menuHeight - 6)
            root.sortMenu.popup(root.overlayParent, point)
        }

        contentItem: Text {
            text: "Orden: " + sortButton.text
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.textScale)
            font.weight: CelestinaTheme.weightMedium
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }

        background: GlassPill {
            backdrop: root.bottomView
            floating: root.bottomFloating
            fill: sortButton.hovered
                  ? CelestinaTheme.surfaceHover
                  : CelestinaTheme.controlFill
            border.width: sortButton.activeFocus ? 1 : 0
            border.color: CelestinaTheme.focusRing
        }
    }

    NavButton {
        id: sortDirectionButton
        Layout.alignment: Qt.AlignVCenter
        iconName: root.controller.sortAscending
                  ? "view-sort-ascending"
                  : "view-sort-descending"
        fallbackIcon: root.controller.sortAscending
                      ? "view-sort-ascending"
                      : "view-sort-descending"
        helpText: root.controller.sortAscending
                  ? "Orden ascendente"
                  : "Orden descendente"
        onClicked: root.controller.toggleSortDirection()
    }

    // Lista / Cuadrícula / Detalles: tres pastillas independientes, no un
    // segmentado dentro de una cápsula. Sigue siendo una sola elección — lo
    // dice el relleno del modo activo, no una caja alrededor de los tres.
    Item {
        id: viewSeg
        Layout.preferredHeight: 30
        Layout.preferredWidth: viewSegRow.implicitWidth

        Row {
            id: viewSegRow
            anchors.centerIn: parent
            spacing: 6

            Repeater {
                model: [
                    { mode: "list", label: "Lista" },
                    { mode: "grid", label: "Cuadrícula" },
                    { mode: "details", label: "Detalles" }
                ]

                delegate: GlassPill {
                    id: seg
                    required property var modelData
                    readonly property bool active: root.panel.viewMode === modelData.mode
                    width: segLabel.implicitWidth + 22
                    height: 30
                    backdrop: root.bottomView
                    floating: root.bottomFloating
                    fill: seg.active ? CelestinaTheme.surfaceSelected
                          : segMouse.containsMouse ? CelestinaTheme.surfaceHover
                          : CelestinaTheme.controlFill

                    Accessible.role: Accessible.RadioButton
                    Accessible.name: "Vista " + seg.modelData.label
                    Accessible.checked: seg.active

                    Text {
                        id: segLabel
                        anchors.centerIn: parent
                        text: seg.modelData.label
                        color: seg.active ? CelestinaTheme.text : CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.textScale)
                        font.weight: seg.active ? CelestinaTheme.weightMedium
                                                : CelestinaTheme.weightRegular
                    }

                    MouseArea {
                        id: segMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            root.panel.viewMode = seg.modelData.mode
                            root.panel.persist()
                        }
                    }
                }
            }
        }
    }

    BusyIndicator {
        id: busy
        Layout.preferredWidth: 26
        Layout.preferredHeight: 26
        running: root.controller.loading
        visible: running
    }
}
