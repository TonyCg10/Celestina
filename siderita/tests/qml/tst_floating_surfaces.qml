import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// Las superficies flotantes reales sobre un contenido que imita al delegado de
// fila: MouseArea de tres botones con hover y un DragHandler que se lleva un
// agarre pasivo en la pulsación. Cada caso pincha, pasa por encima y barre
// sobre la caja, y comprueba que nada de eso llega a la fila de debajo.
TestCase {
    id: testCase
    name: "FloatingSurfaces"
    width: 600
    height: 400
    visible: true
    when: windowShown

    property int contentClicks: 0
    property int contentRightClicks: 0
    property int contentMiddleClicks: 0
    property bool contentDragged: false

    MouseArea {
        id: contentMouse
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        hoverEnabled: true
        onClicked: function(mouse) {
            if (mouse.button === Qt.RightButton)
                testCase.contentRightClicks++
            else if (mouse.button === Qt.MiddleButton)
                testCase.contentMiddleClicks++
            else
                testCase.contentClicks++
        }

        DragHandler {
            target: null
            dragThreshold: 8
            onActiveChanged: if (active) testCase.contentDragged = true
        }
    }

    // ── Dobles de las dependencias que inyecta la vista de carpeta ──────────
    QtObject {
        id: controllerStub

        property int sortField: 0
        property bool sortAscending: true
        property string currentPath: "/home/prueba"
        property var phoneMounts: []
        property var phoneNames: []
        property bool searchActive: false
        property bool searchRunning: false
        property string searchQuery: ""
        property string searchSummary: "3 resultados"
        property int sortChanges: 0

        function changeSortField(field) { sortField = field; sortChanges++ }
        function toggleSortDirection() { sortAscending = !sortAscending }
        function displayLocationName(path) { return path }
        function applyQuery(text) { }
        function closeSearch() { }
        function searchRecursive(text) { }
        function cancelSearch() { }
    }

    ListModel {
        id: tabsModelStub
        ListElement { title: "documentos" }
        ListElement { title: "descargas" }
    }

    QtObject {
        id: hostWindowStub
        property real interfaceTextScale: 1.0
        property real interfaceIconScale: 1.0
        property real contentTextScale: 1.0
        property var tabsModel: tabsModelStub
        property int currentTabIndex: 0
        property int closedTabs: 0

        function selectTab(index) { currentTabIndex = index }
        function closeTab(index) { closedTabs++ }
        function openTab(path, foreground) { }
    }

    // La geometría de columnas que la lista publica para la cabecera.
    QtObject {
        id: viewStub
        property real detailsNameX: 40
        property real colSizeW: 80
        property real colDateW: 120
        property real colTypeW: 80
    }

    function init() {
        contentClicks = 0
        contentRightClicks = 0
        contentMiddleClicks = 0
        contentDragged = false
        hoverLeaked = false
        mouseMove(testCase, 10, 380)
    }

    // Recorre una caja: hover, los tres clics y un barrido de arrastre. El
    // hover se anota al entrar — el barrido puede terminar fuera de la caja, y
    // ahí el contenido vuelve a ser suyo con toda razón.
    property bool hoverLeaked: false

    function pokeAt(surface, px, py) {
        mouseMove(surface, px, py)
        hoverLeaked = contentMouse.containsMouse
        mouseClick(surface, px, py, Qt.LeftButton)
        mouseClick(surface, px, py, Qt.RightButton)
        mouseClick(surface, px, py, Qt.MiddleButton)
        mousePress(surface, px, py, Qt.LeftButton)
        mouseMove(surface, px + 30, py + 2)
        mouseMove(surface, px + 70, py + 4)
        mouseRelease(surface, px + 70, py + 4, Qt.LeftButton)
    }

    function verifyContentUntouched(what) {
        verify(!hoverLeaked, what + ": encendió la fila de debajo")
        compare(contentClicks, 0, what + ": el clic izquierdo llegó al contenido")
        compare(contentRightClicks, 0, what + ": el clic derecho llegó al contenido")
        compare(contentMiddleClicks, 0, what + ": el clic central llegó al contenido")
        verify(!contentDragged, what + ": el barrido arrastró el contenido")
    }

    InfoPill {
        id: infoPill
        x: 20
        y: 20
        backdrop: null
        text: "Papelera  ·  3"
    }

    function test_info_pill_owns_its_pointer() {
        pokeAt(infoPill, 20, 15)
        verifyContentUntouched("InfoPill")
    }

    DetailsHeader {
        id: detailsHeader
        x: 20
        y: 80
        width: 400
        height: 26
        controller: controllerStub
        view: viewStub
    }

    // El canal de la izquierda (antes de la primera columna) y el margen
    // derecho no tienen MouseArea de título: eran la fuga de esta tira.
    function test_details_header_owns_its_pointer() {
        pokeAt(detailsHeader, 8, 13)
        verifyContentUntouched("DetailsHeader")
        compare(controllerStub.sortChanges, 0,
                "el canal izquierdo ordenó por una columna")
    }

    function test_details_header_still_sorts() {
        const before = controllerStub.sortChanges
        mouseClick(detailsHeader, viewStub.detailsNameX + 200, 13, Qt.LeftButton)
        verify(controllerStub.sortChanges > before,
               "los títulos de columna dejaron de ordenar")
    }

    TopBar {
        id: topBar
        x: 20
        y: 140
        width: 500
        height: CelestinaTheme.controlHeightLg
        controller: controllerStub
        activeView: null
        hostWindow: hostWindowStub
        overlayParent: testCase
        pathMenu: null
    }

    function test_top_bar_path_pill_owns_its_pointer() {
        pokeAt(topBar, 60, topBar.height / 2)
        verifyContentUntouched("TopBar (ruta)")
    }

    function test_top_bar_search_pill_owns_its_pointer() {
        pokeAt(topBar, topBar.width - 8, topBar.height / 2)
        verifyContentUntouched("TopBar (búsqueda)")
    }

    // La pastilla de ruta tiene MouseArea propio sin `preventStealing`, así que
    // era la misma puerta que la tarjeta del modal: pulsar dentro y salir hacia
    // la lista dejaba que el handler de la fila se llevara el agarre.
    function test_top_bar_sweep_leaving_the_pill() {
        mousePress(topBar, 60, topBar.height / 2, Qt.LeftButton)
        mouseMove(topBar, 70, topBar.height / 2 + 6)
        mouseMove(topBar, 120, topBar.height / 2 + 40)
        mouseMove(topBar, 220, topBar.height / 2 + 120)
        mouseRelease(topBar, 220, topBar.height / 2 + 120, Qt.LeftButton)
        verify(!contentDragged, "salir de la pastilla de ruta arrastró el contenido")
    }

    TabStrip {
        id: tabStrip
        x: 20
        y: 220
        width: 400
        height: 36
        controller: controllerStub
        hostWindow: hostWindowStub
        topBar: topBar
        active: true
    }

    // La pestaña sólo aceptaba izquierdo y central, así que el clic derecho
    // abría el menú del archivo tapado y el barrido lo arrastraba.
    function test_tab_chip_owns_its_pointer() {
        pokeAt(tabStrip, 40, 18)
        verifyContentUntouched("TabStrip")
    }

    function test_tab_chip_still_selects() {
        hostWindowStub.currentTabIndex = 0
        mouseClick(tabStrip, 250, 18, Qt.LeftButton)
        compare(hostWindowStub.currentTabIndex, 1,
                "la pestaña dejó de poder seleccionarse")
    }
}
