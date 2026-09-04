import QtQuick
import org.celestina.siderita 1.0

// ─── TrashHeader ────────────────────────────────────────────────────────────
// Cabecera de la ubicación "Papelera": una pastilla con el recuento y las
// acciones en bloque (vaciar con confirmación en dos pasos, restaurar todo,
// volver). El estado de la confirmación es interno. Controlador, cristal de
// fondo y escala de texto por propiedad; la vista de carpeta la posiciona y la
// muestra según `trashActive`.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    property var controller
    property Item backdrop     // topBar.activeView: cristal de la pastilla
    property real textScale: 1.0

    property bool confirmingEmpty: false
    onVisibleChanged: if (!visible) confirmingEmpty = false

    InfoPill {
        textScale: root.textScale
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        backdrop: root.backdrop
        iconName: "user-trash"
        iconFallback: "user-trash"
        maxWidth: root.width - trashHeaderControls.width - 10
        // "Empty" is an answer, not the absence of one: while the Trash is
        // still being read there is no count yet, and the pill says nothing.
        text: "Papelera" + (root.controller.trashNames.length > 0
                            ? "  ·  " + root.controller.trashNames.length
                            : root.controller.loading ? "" : "  ·  vacía")
    }

    // Los botones flotan directamente sobre la lista y no viven dentro de
    // ninguna caja: sin este envoltorio, pulsar uno y arrastrar hacia el
    // contenido arrancaba el arrastre del archivo que tapan.
    Item {
        id: trashHeaderControls
        anchors.right: parent.right
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        width: trashHeaderControlsRow.width
        height: trashHeaderControlsRow.height

        CelestinaInputShield { }

        Row {
            id: trashHeaderControlsRow
            spacing: 8

            // Its own pill too: the warning floats over the trash listing, so
            // it needs a surface to be readable on.
            InfoPill {
                textScale: root.textScale
                visible: root.confirmingEmpty
                anchors.verticalCenter: parent.verticalCenter
                backdrop: root.backdrop
                text: "¿Vaciar? No se puede deshacer"
            }
            CelestinaIconButton {
                iconName: root.confirmingEmpty ? "check" : "user-trash"
                helpText: root.confirmingEmpty ? "Vaciar definitivamente"
                                               : "Vaciar la papelera"
                role: CelestinaButton.Destructive
                visible: root.controller.trashNames.length > 0
                onClicked: {
                    if (root.confirmingEmpty) {
                        root.controller.emptyTrash()
                        root.confirmingEmpty = false
                    } else {
                        root.confirmingEmpty = true
                    }
                }
            }
            // Stays in place while the confirmation is up: hiding it slid the
            // back button under the pointer that was about to press it.
            CelestinaIconButton {
                iconName: "rotate-ccw"
                helpText: "Restaurar todo"
                visible: root.controller.trashNames.length > 0
                enabled: !root.confirmingEmpty
                onClicked: root.controller.restoreAllTrash()
            }
            CelestinaIconButton {
                iconName: "go-previous"
                helpText: "Volver"
                role: CelestinaButton.Primary
                onClicked: root.controller.closeTrash()
            }
        }
    }
}
