import QtQuick
import org.celestina.siderita 1.0

// ─── SidebarChevron ───────────────────────────────────────────────────────────
// El galón Lucide de las cabeceras plegables. Una sola forma gira para contar
// el cambio de estado sin depender de métricas tipográficas de un carácter.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    property real iconScale: 1.0
    property bool collapsed: false

    width: Math.round(CelestinaTheme.iconSm * iconScale)
    height: width

    CelestinaIcon {
        anchors.fill: parent
        name: "chevron-down"
        fallbackName: "chevron-down"
        tone: CelestinaIcon.Secondary
        transformOrigin: Item.Center
        rotation: parent.collapsed ? -90 : 0

        Behavior on rotation {
            NumberAnimation {
                duration: CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }
    }
}
