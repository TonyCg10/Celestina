import QtQuick
import QtQuick.Shapes

// ─── CelestinaFileIcon ────────────────────────────────────────────────────────
// El icono de un tipo de contenido que no es carpeta: documento, imagen, código,
// comprimido, disco… Igual que `CelestinaFolderIcon`, el icono **es** la forma —
// un dibujo relleno con el lavado de color de su tono, no un trazo teñido.
//
// La geometría sale de `CelestinaIconShapes`, que es tabla generada; aquí sólo
// se escala a la caja y se rellena. Nada de máscaras: el `Shape` se rasteriza al
// tamaño final, así que a 16 px el borde es tan limpio como a 64.
//
// Si el nombre no está en la tabla, `known` es falso y el consumidor se queda
// con el glifo de trazo de siempre. Esa decisión no es de este componente: sólo
// dice si sabe dibujarlo.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: icon

    // Nombre semántico del catálogo compartido (`text-x-generic`, `file-code`…).
    property string name: ""
    property color tone: CelestinaTheme.glyphFile
    property color gradientTop: CelestinaTheme.iconGradientTop(tone)
    property color gradientBottom: CelestinaTheme.iconGradientBottom(tone)

    // By its own name first: the shape table covers families the stroke
    // catalogue does not publish — a page per language — and asking `resolve`
    // for those would have degraded them to the generic page. With no shape of
    // that name it falls back to the same `resolve` the stroke glyphs use, so a
    // legacy alias (`text-x-generic`) still finds its own without a second table
    // of synonyms to keep.
    readonly property var paths: {
        const own = CelestinaIconShapes.pathsFor(name)
        return own.length > 0 ? own
                              : CelestinaIconShapes.pathsFor(CelestinaIcons.resolve(name, ""))
    }
    readonly property bool known: paths.length > 0
    readonly property real side: Math.min(width, height)
    readonly property real scaleFactor: side / CelestinaIconShapes.viewBox

    implicitWidth: CelestinaTheme.iconMd
    implicitHeight: CelestinaTheme.iconMd

    // Los caminos vienen en la rejilla de 256 del catálogo, así que el dibujo se
    // escala entero en vez de re-generar cada camino por tamaño.
    Shape {
        width: CelestinaIconShapes.viewBox
        height: CelestinaIconShapes.viewBox
        visible: icon.known
        preferredRendererType: Shape.CurveRenderer
        transform: Scale {
            xScale: icon.scaleFactor
            yScale: icon.scaleFactor
        }

        // Dos trazos cubren el catálogo (un disco y una pantalla llevan dos
        // piezas; el resto, una). Un hueco vacío no dibuja nada, así que la
        // forma de una sola pieza no paga por la segunda.
        ShapePath {
            strokeWidth: 0
            fillGradient: LinearGradient {
                x1: 0
                y1: 0
                x2: 0
                y2: CelestinaIconShapes.viewBox
                GradientStop { position: 0; color: icon.gradientTop }
                GradientStop { position: 1; color: icon.gradientBottom }
            }
            PathSvg { path: icon.paths.length > 0 ? icon.paths[0] : "" }
        }

        ShapePath {
            strokeWidth: 0
            fillGradient: LinearGradient {
                x1: 0
                y1: 0
                x2: 0
                y2: CelestinaIconShapes.viewBox
                GradientStop { position: 0; color: icon.gradientTop }
                GradientStop { position: 1; color: icon.gradientBottom }
            }
            PathSvg { path: icon.paths.length > 1 ? icon.paths[1] : "" }
        }
    }
}
