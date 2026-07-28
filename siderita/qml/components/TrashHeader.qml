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
        text: "Papelera" + (root.controller.trashNames.length > 0
                            ? "  ·  " + root.controller.trashNames.length : "  ·  vacía")
    }

    Row {
        id: trashHeaderControls
        anchors.right: parent.right
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
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
        CelestinaButton {
            text: root.confirmingEmpty ? "Vaciar definitivamente" : "Vaciar"
            destructive: true
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
        CelestinaButton {
            text: "Restaurar todo"
            visible: root.controller.trashNames.length > 0 && !root.confirmingEmpty
            onClicked: root.controller.restoreAllTrash()
        }
        CelestinaButton {
            text: "Volver"
            primary: true
            onClicked: root.controller.closeTrash()
        }
    }
}
