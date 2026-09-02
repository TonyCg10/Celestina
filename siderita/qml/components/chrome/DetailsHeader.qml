import QtQuick
import QtQuick.Layouts
import org.celestina.siderita 1.0

// ─── DetailsHeader ──────────────────────────────────────────────────────────
// La cabecera de columnas de la vista de detalles: una tira tonal alineada
// a las columnas de la lista; cada título ordena por su campo (un segundo clic
// en el activo invierte el sentido) y lleva un indicador Lucide. La geometría de las
// columnas viene de la lista (`view`), el orden del controlador y la escala del
// texto por propiedad. La vista de carpeta la posiciona y la muestra.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    property var controller
    property var view          // fileList: geometría de columnas
    property real textScale: 1.0

    // La tira se pinta sobre la lista: los títulos tienen su propio MouseArea,
    // pero el canal de la izquierda y el margen derecho no, y por ahí seguía
    // pasando el puntero a la primera fila.
    CelestinaInputShield { }

    Rectangle {
        anchors.fill: parent
        radius: CelestinaTheme.radiusSm
        color: CelestinaTheme.controlFill
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.divider
    }

    RowLayout {
        x: root.view.detailsNameX - 4
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width - x - 16
        // Sin altura propia la fila medía 0 —sus celdas sólo llevan `Layout`, y
        // el `Row` de dentro va anclado, que no aporta tamaño implícito—, así
        // que las áreas de clic de los títulos eran de altura cero y ordenar
        // pinchando una columna nunca llegó a funcionar.
        height: parent.height
        spacing: 12

        Repeater {
            model: [
                { label: "Nombre", field: 0, w: -1, align: Text.AlignLeft },
                { label: "Tamaño", field: 1, w: root.view.colSizeW, align: Text.AlignRight },
                { label: "Fecha", field: 2, w: root.view.colDateW, align: Text.AlignLeft },
                { label: "Tipo", field: 3, w: root.view.colTypeW, align: Text.AlignLeft }
            ]

            delegate: Item {
                id: hcell
                required property var modelData
                readonly property bool activeSort: root.controller.sortField === modelData.field
                Layout.fillWidth: modelData.w < 0
                Layout.preferredWidth: modelData.w < 0 ? 60 : modelData.w
                Layout.fillHeight: true

                // The hover fill its sibling lists have: a title that can be
                // pressed says so before it is.
                Rectangle {
                    anchors.fill: parent
                    radius: CelestinaTheme.radiusSm
                    color: hcellMouse.containsMouse
                           ? CelestinaTheme.surfaceHover : CelestinaTheme.clear

                    Behavior on color {
                        ColorAnimation { duration: CelestinaTheme.motionFast }
                    }
                }

                Row {
                    anchors.left: hcell.modelData.align === Text.AlignLeft
                                  ? parent.left : undefined
                    anchors.right: hcell.modelData.align === Text.AlignRight
                                   ? parent.right : undefined
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: CelestinaTheme.spaceXs

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: hcell.modelData.label
                        color: hcell.activeSort ? CelestinaTheme.text
                                                : CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: Math.round(CelestinaTheme.fontCaption
                                                   * root.textScale)
                        font.weight: CelestinaTheme.weightDemiBold
                    }

                    CelestinaIcon {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.round(CelestinaTheme.iconSm * root.textScale)
                        height: width
                        visible: hcell.activeSort
                        name: root.controller.sortAscending
                              ? "view-sort-ascending" : "view-sort-descending"
                        fallbackName: "arrow-down"
                        tone: CelestinaIcon.Primary
                    }
                }

                MouseArea {
                    id: hcellMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        if (hcell.activeSort)
                            root.controller.toggleSortDirection()
                        else
                            root.controller.changeSortField(hcell.modelData.field)
                    }
                }
            }
        }
    }
}
