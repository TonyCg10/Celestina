import QtQuick
import org.celestina.siderita 1.0

// ─── InfoPill ─────────────────────────────────────────────────────────────────
// Una pastilla de cristal que se ajusta a su propia etiqueta. Las barras
// flotantes (el resumen de búsqueda, la cabecera de la papelera) se construyen
// con éstas en vez de con una barra que cruza la ventana, así que una cabecera
// se lee como varias pastillas independientes y el contenido asoma entre ellas.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    // La escala de texto de la interfaz, traída por quien lo usa.
    property real textScale: 1.0

    id: infoPill

    required property Item backdrop
    property alias text: pillLabel.text
    property string iconName: ""
    property string iconFallback: "file"
    // Ceiling for a label that would otherwise outgrow its strip; the text
    // elides inside it. Unset (≤ 0) means "as wide as the text needs".
    property int maxWidth: -1

    readonly property int naturalWidth:
            (pillIcon.visible ? 14 + pillIcon.width + 10 : 14)
            + Math.ceil(pillLabel.implicitWidth) + 14

    implicitHeight: 30
    height: implicitHeight
    width: maxWidth > 0 ? Math.min(naturalWidth, maxWidth) : naturalWidth

    // Una pastilla es opaca sobre la lista: sin esto el puntero seguía llegando
    // a la fila que tapa (hover, los tres clics y el arrastre de archivo).
    CelestinaInputShield { }

    GlassSurface {
        anchors.fill: parent
        backdropSource: infoPill.backdrop
        captureEnabled: infoPill.visible
        liveCapture: true
        cornerRadius: CelestinaTheme.radiusPill
    }

    CelestinaIcon {
        id: pillIcon
        visible: infoPill.iconName.length > 0
        anchors.left: parent.left
        anchors.leftMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        width: CelestinaTheme.iconSm
        height: CelestinaTheme.iconSm
        name: infoPill.iconName
        fallbackName: infoPill.iconFallback
    }

    Text {
        id: pillLabel
        anchors.left: pillIcon.visible ? pillIcon.right : parent.left
        anchors.leftMargin: pillIcon.visible ? 10 : 14
        anchors.right: parent.right
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: Math.round(CelestinaTheme.fontRowSecondary * textScale)
        font.weight: CelestinaTheme.weightMedium
        // Tabular figures so the counts in these floating strips (search summary,
        // trash header) keep an even width as they change.
        font.features: CelestinaTheme.fontFeaturesTabular
        elide: Text.ElideRight
    }
}
