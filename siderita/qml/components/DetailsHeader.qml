import QtQuick
import QtQuick.Layouts
import org.celestina.siderita 1.0

// ─── DetailsHeader ──────────────────────────────────────────────────────────
// La cabecera de columnas de la vista de detalles: una tira de cristal alineada
// a las columnas de la lista; cada título ordena por su campo (un segundo clic
// en el activo invierte el sentido) y lleva una flecha ↑/↓. La geometría de las
// columnas viene de la lista (`view`), el orden del controlador y la escala del
// texto por propiedad. La vista de carpeta la posiciona y la muestra.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    property var controller
    property var view          // fileList: geometría de columnas + cristal
    property real textScale: 1.0

    GlassSurface {
        anchors.fill: parent
        backdropSource: root.view
        captureEnabled: root.visible
        liveCapture: true
        cornerRadius: CelestinaTheme.radiusSm
    }

    Rectangle {
        anchors.fill: parent
        radius: CelestinaTheme.radiusSm
        color: "transparent"
        border.width: 1
        border.color: CelestinaTheme.dividerStrong
    }

    RowLayout {
        x: root.view.detailsNameX - 4
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width - x - 16
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

                Text {
                    anchors.fill: parent
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: hcell.modelData.align
                    text: hcell.modelData.label
                          + (hcell.activeSort
                             ? (root.controller.sortAscending ? "  ↑" : "  ↓") : "")
                    color: hcell.activeSort ? CelestinaTheme.text
                                            : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.textScale)
                    font.weight: CelestinaTheme.weightDemiBold
                    elide: Text.ElideRight
                }

                MouseArea {
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
