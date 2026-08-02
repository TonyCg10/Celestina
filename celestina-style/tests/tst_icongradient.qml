import QtQuick
import QtTest
import CelestinaStyle

// La receta de color de los iconos de contenido. Lo que se prueba aquí es
// justo lo que se rompió a mano: derivar el lavado en HSL metía oliva en los
// tonos cálidos. En OKLCH el tono percibido se mantiene, y eso es comprobable
// sin GPU — los píxeles del dibujo son otra cosa y necesitan sesión real.
TestCase {
    id: testCase

    name: "IconGradient"
    when: windowShown

    readonly property var tones: [
        CelestinaTheme.glyphDirectory,
        CelestinaTheme.glyphFile,
        CelestinaTheme.glyphSymlink,
        CelestinaTheme.glyphNavigation,
        CelestinaTheme.glyphDevice,
        CelestinaTheme.glyphAccentBlue,
        CelestinaTheme.glyphAccentCyan,
        CelestinaTheme.glyphAccentGreen,
        CelestinaTheme.glyphAccentViolet,
        CelestinaTheme.glyphAccentCoral,
        CelestinaTheme.glyphAccentAmber,
        CelestinaTheme.favorite,
        CelestinaTheme.danger
    ]

    function hueGap(first, second) {
        const gap = Math.abs(first - second) % 360
        return gap > 180 ? 360 - gap : gap
    }

    CelestinaFolderIcon {
        id: folder
        width: 64
        height: 64
    }

    // El viaje de ida y vuelta por OKLCH no puede mover el color: si esto falla,
    // toda la receta está construida sobre arena.
    function test_oklch_round_trip_is_faithful() {
        for (const tone of tones) {
            const lch = CelestinaTheme.toOklch(tone)
            const back = CelestinaTheme.fromOklch(lch[0], lch[1], lch[2], tone.a)
            fuzzyCompare(back.r, tone.r, 0.01)
            fuzzyCompare(back.g, tone.g, 0.01)
            fuzzyCompare(back.b, tone.b, 0.01)
        }
    }

    // El caso que motivó todo esto: el extremo bajo se oscurece **sin cambiar
    // de familia**. Con la derivación en HSL, un ámbar terminaba en verde oliva
    // — un giro de tono percibido enorme.
    function test_the_bottom_end_stays_in_its_own_hue() {
        for (const tone of tones) {
            const base = CelestinaTheme.toOklch(tone)
            const top = CelestinaTheme.toOklch(CelestinaTheme.iconGradientTop(tone))
            const bottom = CelestinaTheme.toOklch(
                               CelestinaTheme.iconGradientBottom(tone))

            verify(hueGap(bottom[2], base[2]) <= CelestinaTheme.iconGradientTurn + 3,
                   "el extremo bajo se fue de tono")
            verify(hueGap(top[2], base[2]) <= CelestinaTheme.iconGradientTurn + 3,
                   "el extremo alto se fue de tono")
            verify(top[0] > base[0], "el extremo alto no ilumina")
            verify(bottom[0] < base[0], "el extremo bajo no asienta")
        }
    }

    // Suave: un lavado que se lea como dos colores distintos deja de ser un
    // material y pasa a ser un cartel.
    function test_the_wash_stays_soft() {
        for (const tone of tones) {
            const top = CelestinaTheme.toOklch(CelestinaTheme.iconGradientTop(tone))
            const bottom = CelestinaTheme.toOklch(
                               CelestinaTheme.iconGradientBottom(tone))
            const spread = top[0] - bottom[0]
            verify(spread > 0.02, "el lavado es invisible")
            verify(spread < 0.28, "el lavado grita")
        }
    }

    // El fondo de la carpeta acompaña al bolsillo: mismo tono, más asentado.
    function test_backdrop_follows_its_pocket() {
        for (const tone of tones) {
            const base = CelestinaTheme.toOklch(tone)
            const backdrop = CelestinaTheme.toOklch(
                                 CelestinaTheme.iconBackdropTone(tone))
            verify(backdrop[0] < base[0], "el fondo no asienta")
            verify(hueGap(backdrop[2], base[2]) < 2, "el fondo cambió de tono")
        }
    }

    // La tinta del emblema y la hoja no son colores fijos: forman par con lo
    // que tienen debajo, y sobre un tono claro cambian a una lámina profunda.
    // Sin esto, un favorito ámbar se quedaba con un emblema invisible.
    function test_ink_pairs_with_what_it_sits_on() {
        for (const tone of tones) {
            const pocket = CelestinaTheme.toOklch(
                               CelestinaTheme.iconGradientBottom(tone))
            const ink = CelestinaTheme.toOklch(CelestinaTheme.iconEmblemInk(tone))
            verify(Math.abs(ink[0] - pocket[0]) > 0.2,
                   "el emblema no se separa de su bolsillo")

            const backdrop = CelestinaTheme.toOklch(
                                 CelestinaTheme.iconBackdropTone(tone))
            const paper = CelestinaTheme.toOklch(CelestinaTheme.iconSheetTone(tone))
            verify(Math.abs(paper[0] - backdrop[0]) > 0.2,
                   "la hoja no se separa del fondo")
        }
    }

    // La opacidad es del tono, no de la receta.
    function test_alpha_survives() {
        const translucent = CelestinaTheme.withAlpha(
                                CelestinaTheme.glyphDirectory, 0.4)
        fuzzyCompare(CelestinaTheme.iconGradientTop(translucent).a, 0.4, 0.02)
        fuzzyCompare(CelestinaTheme.iconGradientBottom(translucent).a, 0.4, 0.02)
        fuzzyCompare(CelestinaTheme.iconBackdropTone(translucent).a, 0.4, 0.02)
    }

    // El componente deriva solo: dar un tono basta, y los tres colores del
    // dibujo salen de la receta sin que el consumidor decida nada.
    function test_the_folder_derives_everything_from_one_tone() {
        folder.tone = CelestinaTheme.glyphAccentCoral
        compare(folder.gradientTop.toString(),
                CelestinaTheme.iconGradientTop(
                    CelestinaTheme.glyphAccentCoral).toString())
        compare(folder.gradientBottom.toString(),
                CelestinaTheme.iconGradientBottom(
                    CelestinaTheme.glyphAccentCoral).toString())
        compare(folder.backdropTone.toString(),
                CelestinaTheme.iconBackdropTone(
                    CelestinaTheme.glyphAccentCoral).toString())
        folder.tone = CelestinaTheme.glyphDirectory
    }

    // El catálogo de formas comparte el espacio de nombres con el de trazos: un
    // alias heredado tiene que encontrar su dibujo sin una segunda tabla de
    // sinónimos, que es como se desincronizan dos catálogos.
    function test_shapes_share_the_glyph_naming_contract() {
        const aliases = [
            { name: "text-x-generic",       shape: "file" },
            { name: "image-x-generic",      shape: "file-image" },
            { name: "audio-x-generic",      shape: "file-music" },
            { name: "video-x-generic",      shape: "file-video-camera" },
            { name: "emblem-symbolic-link", shape: "symlink" }
        ]
        for (const item of aliases) {
            const resolved = CelestinaIcons.resolve(item.name, "")
            compare(resolved, item.shape, item.name + ": resolvió a " + resolved)
            verify(CelestinaIconShapes.has(resolved),
                   item.name + ": sin forma para " + resolved)
        }
    }

    // Cada forma del catálogo tiene geometría de verdad, y ninguna se pasa de
    // los dos trazos que el componente sabe pintar.
    function test_every_shape_is_drawable() {
        for (const name in CelestinaIconShapes.paths) {
            const paths = CelestinaIconShapes.pathsFor(name)
            verify(paths.length > 0, name + ": sin caminos")
            verify(paths.length <= 2,
                   name + ": " + paths.length + " caminos, y el componente pinta 2")
            for (const path of paths)
                verify(path.length > 20, name + ": camino sospechosamente corto")
        }
    }

    // Un nombre que no está en la tabla no dibuja nada: es la señal que usa el
    // consumidor para quedarse con el glifo de trazo.
    function test_an_unknown_name_draws_nothing() {
        compare(CelestinaIconShapes.pathsFor("no-existe").length, 0)
        compare(CelestinaIconShapes.has("no-existe"), false)
    }

    // La geometría es fracción del lado, así que el mismo dibujo vale a 16 y a
    // 128: si esto deja de cumplirse, el icono se rompe en la lista o en la
    // cuadrícula, no en las dos a la vez.
    function test_geometry_scales_with_the_side() {
        const sizes = [16, 20, 24, 48, 128]
        for (const size of sizes) {
            folder.width = size
            folder.height = size
            fuzzyCompare(folder.edgeRight - folder.edgeLeft, size * 0.88, 0.5)
            verify(folder.pocketTop > folder.sheetTop)
            verify(folder.sheetTop > folder.bodyTop)
            verify(folder.edgeBottom > folder.pocketTop)
        }
        folder.width = 64
        folder.height = 64
    }
}
