import QtQuick
import QtTest
import "../qml" as Desktop

TestCase {
    id: testCase

    name: "OutputChooser"

    function screen(name, width) {
        return {
            "name": name,
            "width": width,
            "height": 1080,
            "devicePixelRatio": 1
        }
    }

    Desktop.OutputChooser {
        id: chooser

        visible: false
        reducedMotion: true
        screens: []
    }

    // El mismo diálogo con el alto que un compositor de mosaico puede imponer,
    // muy por debajo del que la tarjeta pide. La fila de pantallas tiene alto
    // propio y los botones van anclados al pie, así que sin acotarla se montaban
    // uno encima del otro.
    Desktop.OutputChooser {
        id: squeezed

        visible: false
        reducedMotion: true
        screens: [testCase.screen("DP-1", 1920), testCase.screen("DP-2", 2560)]
        height: 220
    }

    function init() {
        chooser.chosen = ""
        chooser.cancelled = false
        chooser.screens = [screen("DP-1", 1920),
                           screen("DP-2", 2560),
                           screen("HDMI-A-1", 1920)]
        chooser.selectOutput(1)
        compare(chooser.selectedOutputName, "DP-2")
    }

    // The same question serves two reasons: sharing with an application, and
    // recording to show a bug. The words may change; the window title may
    // not, because the niri rule that floats this dialog matches on it, and
    // changing it would leave the dialog tiled instead.
    function test_the_words_change_but_the_window_title_does_not() {
        compare(chooser.headline, qsTr("Compartir pantalla"));
        compare(chooser.confirmText, qsTr("Compartir"));
        compare(chooser.title, qsTr("Compartir pantalla"));

        chooser.headline = qsTr("Grabar pantalla");
        chooser.prompt = qsTr("Elige qué salida se grabará.");
        chooser.confirmText = qsTr("Grabar");

        compare(chooser.title, qsTr("Compartir pantalla"));
        chooser.headline = qsTr("Compartir pantalla");
        chooser.prompt = Qt.binding(() => chooser.screens.length > 1
                                          ? qsTr("Elige qué salida verá la aplicación.")
                                          : qsTr("Se compartirá esta salida."));
        chooser.confirmText = qsTr("Compartir");
    }

    function test_reorder_preserves_output_identity() {
        chooser.screens = [screen("HDMI-A-1", 1920),
                           screen("DP-2", 2560),
                           screen("DP-1", 1920)]

        compare(chooser.selected, 1)
        compare(chooser.selectedOutputName, "DP-2")
    }

    function test_removing_an_earlier_output_preserves_selection() {
        chooser.screens = [screen("DP-2", 2560),
                           screen("HDMI-A-1", 1920)]

        compare(chooser.selected, 0)
        compare(chooser.selectedOutputName, "DP-2")
    }

    // Geometría, no lógica: la fila cede alto ante los botones en vez de
    // solaparlos, y no se queda en nada cuando hay sitio de sobra.
    function test_row_yields_to_the_actions_when_the_window_is_short() {
        const row = findChild(squeezed.contentItem, "outputRow")
        const actions = findChild(squeezed.contentItem, "chooserActions")
        verify(row !== null && actions !== null)

        const rowBottom = row.mapToItem(null, 0, row.height).y
        const actionsTop = actions.mapToItem(null, 0, 0).y
        verify(rowBottom <= actionsTop,
               "la fila de pantallas invade los botones: " + rowBottom
               + " > " + actionsTop)
        verify(row.height > 0, "la fila se quedó sin alto")
    }

    function test_row_keeps_its_full_height_when_it_fits() {
        const row = findChild(chooser.contentItem, "outputRow")
        verify(row !== null)
        compare(row.height, chooser.rowHeight,
                "la fila perdió alto en una ventana que sí da de sí")
    }

    function test_removing_selected_output_uses_bounded_fallback() {
        chooser.screens = [screen("DP-1", 1920),
                           screen("HDMI-A-1", 1920)]

        compare(chooser.selected, 1)
        compare(chooser.selectedOutputName, "HDMI-A-1")
    }
}
