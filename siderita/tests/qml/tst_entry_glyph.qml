import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// Qué se pinta por una entrada. La decisión es de datos —carpeta dibujada o
// glifo teñido, y qué emblema lleva el lugar— así que se puede comprobar sin
// GPU; cómo se ve el lavado necesita sesión real.
TestCase {
    id: testCase
    name: "EntryGlyph"
    width: 200
    height: 200
    visible: true
    when: windowShown

    EntryGlyph {
        id: glyph
        width: 48
        height: 48
        kind: "directory"
        path: "/home/prueba/Descargas"
        iconName: "folder-download"
    }

    function init() {
        glyph.width = 48
        glyph.height = 48
        glyph.kind = "directory"
        glyph.iconName = "folder-download"
        glyph.tintOverride = CelestinaTheme.clear
    }

    // Una carpeta es la forma dibujada; lo demás sigue siendo glifo.
    function test_only_directories_get_the_drawn_folder() {
        compare(glyph.isDirectory, true)

        glyph.kind = "file"
        glyph.iconName = "text-x-generic"
        compare(glyph.isDirectory, false)
        compare(glyph.emblemSource.toString(), "",
                "un archivo no puede llevar emblema de carpeta")
    }

    // El emblema traduce el nombre de carpeta tipada al símbolo del lugar: una
    // carpeta dentro de otra carpeta no dice nada.
    function test_typed_folders_carry_their_place_emblem() {
        const cases = [
            { icon: "folder-download",  emblem: "arrow-down" },
            { icon: "folder-music",     emblem: "music" },
            { icon: "folder-pictures",  emblem: "image" },
            { icon: "folder-videos",    emblem: "film" },
            { icon: "folder-documents", emblem: "file-text" },
            { icon: "folder-desktop",   emblem: "monitor" }
        ]
        for (const item of cases) {
            glyph.iconName = item.icon
            const source = glyph.emblemSource.toString()
            verify(source.length > 0, item.icon + ": se quedó sin emblema")
            verify(source.indexOf(item.emblem) >= 0,
                   item.icon + ": esperaba " + item.emblem + " y llegó " + source)
            verify(source.indexOf("folder") < 0,
                   item.icon + ": el emblema es otra carpeta")
        }
    }

    // Una carpeta corriente no lleva nada dentro.
    function test_a_plain_folder_has_no_emblem() {
        glyph.iconName = "folder"
        compare(glyph.emblemSource.toString(), "")
    }

    // El icono que el usuario elige a mano para una carpeta sigue viéndose: se
    // convierte en su emblema en vez de perderse al pasar la carpeta a dibujo.
    function test_a_hand_picked_icon_becomes_the_emblem() {
        glyph.iconName = "star"
        const source = glyph.emblemSource.toString()
        verify(source.length > 0, "el icono elegido a mano se perdió")
        verify(source.indexOf("star") >= 0, "no es el icono elegido: " + source)
    }

    // Los tipos de contenido con forma propia se dibujan; el resto conserva su
    // glifo de trazo, que es lo que impide que un nombre desconocido desaparezca.
    function test_known_file_types_are_drawn_and_the_rest_are_not() {
        glyph.kind = "file"
        const drawn = ["text-x-generic", "image-x-generic", "audio-x-generic",
                       "video-x-generic", "file-code", "file-archive"]
        for (const name of drawn) {
            glyph.iconName = name
            verify(CelestinaIconShapes.has(CelestinaIcons.resolve(name, "")),
                   name + ": esperaba forma propia y no la hay")
        }

        // Un control no es contenido: su sitio es el catálogo de trazos.
        for (const name of ["view-refresh", "settings", "search"]) {
            verify(!CelestinaIconShapes.has(CelestinaIcons.resolve(name, "")),
                   name + ": un icono de control se coló en las formas")
        }
    }

    // La hoja es una franja de dos píxeles en tamaños de lista: por debajo de
    // 24 px se retira sola en vez de ensuciar el dibujo.
    function test_the_sheet_stands_down_when_it_would_be_a_smudge() {
        glyph.width = 48
        glyph.height = 48
        compare(glyph.sheetFits, true)

        glyph.width = 20
        glyph.height = 20
        compare(glyph.sheetFits, false)
    }
}
