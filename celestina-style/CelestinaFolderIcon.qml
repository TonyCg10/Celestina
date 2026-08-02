import QtQuick
import QtQuick.Shapes

// ─── CelestinaFolderIcon ──────────────────────────────────────────────────────
// La carpeta de la suite. El icono **es** la forma: no es un glifo monocromo
// dentro de una caja, sino un dibujo con su relleno y su lavado de color, que es
// lo que le da cuerpo a un tipo de contenido frente a los glifos de interfaz.
//
// Tres partes, como cualquier carpeta física: el fondo con su pestaña, la hoja
// que asoma y el bolsillo delantero, que es donde vive el gradiente. Encima,
// opcionalmente, el emblema que distingue Descargas de Documentos sin cambiar
// el dibujo.
//
// Todo es vectorial y todas las medidas son fracción del lado, así que el mismo
// componente sirve a 16 y a 128 px: un `Shape` se rasteriza al tamaño final, sin
// máscara ni re-muestreo, que es lo que estropea un degradado sobre un trazo.
//
// El color entra por `tone` y de ahí sale todo, con la receta OKLCH del tema —
// nunca con mezclas locales, y nunca aclarando/oscureciendo en HSL, que le mete
// oliva a los tonos cálidos.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: icon

    // El único color obligatorio. Un consumidor puede además fijar los extremos
    // si necesita una carpeta que no salga de la receta (una selección, por
    // ejemplo), pero por defecto no hay nada que decidir.
    property color tone: CelestinaTheme.glyphDirectory
    property color gradientTop: CelestinaTheme.iconGradientTop(tone)
    property color gradientBottom: CelestinaTheme.iconGradientBottom(tone)
    property color backdropTone: CelestinaTheme.iconBackdropTone(tone)
    property color sheetTone: CelestinaTheme.iconSheetTone(tone)

    // La hoja se puede apagar: por debajo de cierto tamaño es una franja de un
    // píxel que sólo ensucia. El consumidor decide, porque es quien sabe a qué
    // tamaño se está pintando.
    property bool sheetVisible: true

    // El emblema del bolsillo. Vacío = carpeta genérica. Se le pasa una fuente
    // ya resuelta (`CelestinaIcons.source(...)`), no un nombre, para que este
    // componente no dependa del catálogo.
    property url emblem: ""
    property real emblemScale: 0.4
    property color emblemInk: CelestinaTheme.iconEmblemInk(tone)

    readonly property real side: Math.min(width, height)

    // Geometría, en fracciones del lado.
    readonly property real inset: side * 0.06
    readonly property real edgeLeft: inset
    readonly property real edgeRight: side - inset
    readonly property real tabTop: side * 0.12
    readonly property real bodyTop: side * 0.26
    readonly property real sheetTop: side * 0.31
    readonly property real pocketTop: side * 0.42
    readonly property real edgeBottom: side * 0.88
    readonly property real corner: side * CelestinaTheme.iconFolderCorner
    readonly property real tabRight:
            edgeLeft + (edgeRight - edgeLeft) * CelestinaTheme.iconFolderTab

    implicitWidth: CelestinaTheme.iconMd
    implicitHeight: CelestinaTheme.iconMd

    // La silueta del fondo — pestaña, hombro y cuerpo — en un solo trazo, para
    // que el hombro sea una curva y no un escalón.
    readonly property string backdropPath: {
        const r = corner
        const run = side * CelestinaTheme.iconFolderShoulder
        return "M " + edgeLeft + " " + (tabTop + r)
             + " A " + r + " " + r + " 0 0 1 " + (edgeLeft + r) + " " + tabTop
             + " L " + (tabRight - run) + " " + tabTop
             + " C " + (tabRight - run * 0.25) + " " + tabTop
             + " "   + (tabRight - run * 0.75) + " " + bodyTop
             + " "   + tabRight + " " + bodyTop
             + " L " + (edgeRight - r) + " " + bodyTop
             + " A " + r + " " + r + " 0 0 1 " + edgeRight + " " + (bodyTop + r)
             + " L " + edgeRight + " " + (edgeBottom - r)
             + " A " + r + " " + r + " 0 0 1 " + (edgeRight - r) + " " + edgeBottom
             + " L " + (edgeLeft + r) + " " + edgeBottom
             + " A " + r + " " + r + " 0 0 1 " + edgeLeft + " " + (edgeBottom - r)
             + " Z"
    }

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            strokeWidth: 0
            fillColor: icon.backdropTone
            PathSvg { path: icon.backdropPath }
        }

        ShapePath {
            strokeWidth: 0
            fillColor: icon.sheetVisible ? icon.sheetTone : CelestinaTheme.clear
            PathRectangle {
                x: icon.edgeLeft + icon.side * 0.06
                y: icon.sheetTop
                width: (icon.edgeRight - icon.edgeLeft) - icon.side * 0.12
                height: icon.pocketTop - icon.sheetTop + icon.side * 0.06
                topLeftRadius: icon.corner * 0.55
                topRightRadius: icon.corner * 0.55
                bottomLeftRadius: 0
                bottomRightRadius: 0
            }
        }

        ShapePath {
            strokeWidth: 0
            fillGradient: LinearGradient {
                x1: 0
                y1: icon.pocketTop
                x2: 0
                y2: icon.edgeBottom
                GradientStop { position: 0; color: icon.gradientTop }
                GradientStop { position: 1; color: icon.gradientBottom }
            }
            PathRectangle {
                x: icon.edgeLeft
                y: icon.pocketTop
                width: icon.edgeRight - icon.edgeLeft
                height: icon.edgeBottom - icon.pocketTop
                topLeftRadius: icon.corner * 0.35
                topRightRadius: icon.corner * 0.35
                bottomLeftRadius: icon.corner
                bottomRightRadius: icon.corner
            }
        }
    }

    CelestinaIcon {
        visible: icon.emblem.toString().length > 0
        x: icon.edgeLeft + (icon.edgeRight - icon.edgeLeft - width) / 2
        y: icon.pocketTop + (icon.edgeBottom - icon.pocketTop - height) / 2
        width: Math.round(icon.side * icon.emblemScale)
        height: width
        source: icon.emblem
        tintOverride: icon.emblemInk
    }
}
