import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// ─── SizePopup ──────────────────────────────────────────────────────────────
// El submenú de tamaños del botón "Tamaño": seis escalas independientes
// (contenido / interfaz / barra lateral, en iconos y texto). Las escalas son de
// la ventana, no de la pestaña, así que la ventana anfitriona (`hostWindow`) y
// la superficie a difuminar (`backdrop`) llegan por propiedad. Quien lo
// instancia fija su posición (x/y) respecto al botón.
// ──────────────────────────────────────────────────────────────────────────────
Popup {
    id: root

    property Item backdrop    // mainPanel: la vista tras el cristal
    property var hostWindow   // la ventana: las seis escalas + persistSizing

    padding: CelestinaTheme.spaceLg
    // Non-modal so the content still scrolls (to watch items resize) while
    // sizes are adjusted; a click outside still closes it via
    // CloseOnPressOutside.
    modal: false
    dim: false
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    // Frosted like the menus and dialogs — glass is the suite's surface
    // language. Samples the view behind it.
    background: GlassCard {
        backdropSource: root.backdrop
        cornerRadius: CelestinaTheme.radiusLg
    }

    contentItem: Column {
        spacing: 6

        CelestinaSectionLabel {
            text: "ICONOS"
        }
        SizeRow {
            label: "Contenido"
            value: root.hostWindow.contentIconScale
            maxValue: 3.0
            onMoved: function(v) {
                root.hostWindow.contentIconScale = v
                root.hostWindow.persistSizing()
            }
        }
        SizeRow {
            label: "Interfaz"
            value: root.hostWindow.interfaceIconScale
            onMoved: function(v) {
                root.hostWindow.interfaceIconScale = v
                root.hostWindow.persistSizing()
            }
        }
        SizeRow {
            label: "Barra lateral"
            value: root.hostWindow.sidebarIconScale
            onMoved: function(v) {
                root.hostWindow.sidebarIconScale = v
                root.hostWindow.persistSizing()
            }
        }

        Item { width: 1; height: 4 }

        CelestinaSectionLabel {
            text: "TEXTO"
        }
        SizeRow {
            label: "Contenido"
            value: root.hostWindow.contentTextScale
            onMoved: function(v) {
                root.hostWindow.contentTextScale = v
                root.hostWindow.persistSizing()
            }
        }
        SizeRow {
            label: "Interfaz"
            value: root.hostWindow.interfaceTextScale
            onMoved: function(v) {
                root.hostWindow.interfaceTextScale = v
                root.hostWindow.persistSizing()
            }
        }
        SizeRow {
            label: "Barra lateral"
            value: root.hostWindow.sidebarTextScale
            onMoved: function(v) {
                root.hostWindow.sidebarTextScale = v
                root.hostWindow.persistSizing()
            }
        }
    }
}
