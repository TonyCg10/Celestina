import QtQuick
import org.celestina.siderita 1.0

// ─── EntryGlyph ───────────────────────────────────────────────────────────────
// Lo que se pinta por una entrada cuando no hay miniatura. Una carpeta es la
// forma dibujada de la suite —con su lavado de color y, si el lugar lo tiene,
// su emblema—; todo lo demás sigue siendo el glifo de Lucide teñido.
//
// La decisión vive aquí y no en cada delegado: la toman la lista, la cuadrícula
// y el selector, y tenerla tres veces era exactamente cómo se desincronizan.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: glyph

    required property string kind
    required property string path
    // El nombre de icono ya resuelto por el panel (respeta la elección del
    // usuario y el tipo de carpeta), y el tinte de acento si lo hay.
    required property string iconName
    property string fallbackName: "file"
    property color tintOverride: CelestinaTheme.clear
    property int tone: CelestinaIcon.File

    readonly property bool isDirectory: kind === "directory"
    // La hoja de la carpeta es una franja de dos píxeles por debajo de 24: a ese
    // tamaño sólo ensucia el dibujo, así que se retira.
    readonly property bool sheetFits: Math.min(width, height) >= 24

    // Los nombres de carpeta tipada de Lucide son carpetas ellos mismos, y una
    // carpeta dentro de otra no dice nada. Aquí se traducen al símbolo que sí
    // distingue el lugar: Descargas es una flecha, Música una nota.
    readonly property var emblems: ({
        "folder-desktop": "monitor",
        "folder-documents": "file-text",
        "folder-download": "arrow-down",
        "folder-music": "music",
        "folder-pictures": "image",
        "folder-videos": "film",
        "folder-publicshare": "share-2",
        "folder-templates": "layout-template",
        "folder-code": "file-code",
        "folder-git-2": "git-branch",
        "folder-heart": "star"
    })

    readonly property url emblemSource: {
        if (!isDirectory)
            return ""
        const mapped = emblems[iconName]
        if (mapped)
            return CelestinaIcons.source(mapped, "file")
        // Un icono que el usuario eligió a mano para esta carpeta no está en la
        // tabla y no es una carpeta tipada: se respeta tal cual como emblema, o
        // el ajuste desaparecería al pasar la carpeta a dibujo.
        if (iconName.length > 0 && iconName !== "folder")
            return CelestinaIcons.source(iconName, "file")
        return ""
    }

    CelestinaFolderIcon {
        anchors.fill: parent
        visible: glyph.isDirectory
        tone: glyph.tintOverride.a > 0 ? glyph.tintOverride
                                       : CelestinaTheme.glyphDirectory
        sheetVisible: glyph.sheetFits
        emblem: glyph.emblemSource
    }

    // Un tipo de contenido con forma propia se dibuja; el catálogo de formas es
    // corto a propósito, así que lo que no está en él —y todo lo que es control—
    // se queda con el glifo de trazo de siempre.
    CelestinaFileIcon {
        id: shape
        anchors.fill: parent
        visible: !glyph.isDirectory && known
        name: glyph.iconName
        tone: glyph.tintOverride.a > 0 ? glyph.tintOverride
              : glyph.kind === "symlink" ? CelestinaTheme.glyphSymlink
                                         : CelestinaTheme.glyphFile
    }

    CelestinaIcon {
        anchors.fill: parent
        visible: !glyph.isDirectory && !shape.known
        name: glyph.iconName
        fallbackName: glyph.fallbackName
        // Un icono elegido a mano suele ser simbólico, y los simbólicos sólo se
        // publican a 16 px: sin pedir el tamaño explícito salen diminutos.
        sourceSize: Qt.size(width, height)
        tone: glyph.tone
        tintOverride: glyph.tintOverride
    }
}
