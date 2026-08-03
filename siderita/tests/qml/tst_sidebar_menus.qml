import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// Los menús de la barra lateral: abrir en pestaña nueva y propiedades. Lugares,
// favoritos y marcadores tenían pestaña; el móvil y los dispositivos no, y de
// propiedades no había en ninguno. Ambas cosas dependen de una ruta en disco,
// que es justo lo que un lugar virtual o un volumen sin montar no tienen.
TestCase {
    id: testCase
    name: "SidebarMenus"
    width: 320
    height: 400
    visible: true
    when: windowShown

    property var openedTabs: []
    property int menuRequests: 0
    property string menuPath: ""

    QtObject {
        id: controllerStub

        property var phoneNames: ["Galaxy"]
        property int phoneRevision: 0
        property var volumeNames: ["USB"]
        property var volumeMounts: ["/run/media/toni/USB"]
        property string currentPath: "/home/toni"
        property int hiddenDeviceCount: 0

        // nombre, id, batería, conectado, montado, punto de montaje — el quinto
        // campo es el que decide si la fila responde al puntero.
        function phoneInfo(index) {
            return ["Galaxy", "abc", "80", "1", "1", "/run/user/1000/magnetita/abc"]
        }
        function hideDevice(name) { }
        function unhideAllDevices() { }
        function openVolume(index) { }
        function openPhone(index) { }
        function hidePlace(key) { }
        function unhideAllPlaces() { }
        function revealPath(path) { }
        function toggleFavorite(path) { }
        function removeBookmark(index) { }
        function moveBookmark(from, to) { }

        property var propertiesFor: []
        function openProperties(path) { propertiesFor.push(path) }
    }

    QtObject {
        id: hostWindowStub

        property var activeController: controllerStub
        property real sidebarIconScale: 1.0
        property real sidebarTextScale: 1.0
        property int sidebarRowHeight: 34

        function openTab(path, foreground) {
            testCase.openedTabs.push({ path: path, foreground: foreground })
        }
    }

    SidebarPhoneSection {
        id: phones
        width: 280
        hostWindow: hostWindowStub
        onContextMenuRequested: function(mountPath, where) {
            testCase.menuRequests++
            testCase.menuPath = mountPath
        }
    }

    function init() {
        // Un menú es modal: si el anterior aún se está cerrando, se queda con
        // los clics del caso siguiente y el fallo parece del móvil.
        menus.closeAll()
        wait(120)
        controllerStub.propertiesFor = []
        openedTabs = []
        menuRequests = 0
        menuPath = ""
    }

    function phoneRowCentre() {
        // La sección pinta cabecera + filas; la fila del teléfono queda bajo la
        // cabecera, así que se apunta a la primera fila real.
        return Qt.point(phones.width / 2, phones.height - 17)
    }

    // El caso reportado: del móvil no salía pestaña con el botón central.
    function test_middle_click_on_a_phone_opens_a_background_tab() {
        const centre = phoneRowCentre()
        mouseClick(phones, centre.x, centre.y, Qt.MiddleButton)

        compare(openedTabs.length, 1, "el botón central no abrió pestaña")
        compare(openedTabs[0].path, "/run/user/1000/magnetita/abc")
        compare(openedTabs[0].foreground, false,
                "una pestaña de botón central se abre detrás")
    }

    // Y el derecho tiene que pedir menú, que antes tampoco existía.
    function test_right_click_on_a_phone_asks_for_its_menu() {
        const centre = phoneRowCentre()
        mouseClick(phones, centre.x, centre.y, Qt.RightButton)

        compare(menuRequests, 1, "el móvil sigue sin menú")
        compare(menuPath, "/run/user/1000/magnetita/abc")
    }

    // El izquierdo conserva lo suyo: navegar en la pestaña actual, sin abrir
    // ninguna nueva.
    function test_left_click_still_navigates_in_place() {
        const centre = phoneRowCentre()
        mouseClick(phones, centre.x, centre.y, Qt.LeftButton)

        compare(openedTabs.length, 0, "el clic normal abrió una pestaña")
    }

    // El menú de dispositivo ofrece la pestaña sólo cuando hay dónde abrirla:
    // un volumen sin montar no tiene ruta todavía.
    SidebarContextMenus {
        id: menus
        hostWindow: hostWindowStub
        overlayParent: testCase
        backdropSource: testCase
    }

    // Propiedades necesita una ruta real, y cada menú la trae de un sitio: el
    // lugar de su clave, el favorito de sí mismo, el marcador de su fila y el
    // dispositivo de su punto de montaje.
    function test_every_sidebar_menu_carries_a_path_for_properties() {
        menus.openPlace("HOME", "Inicio", "/home/toni", Qt.point(10, 10))
        compare(menus.placeTargetPath, "/home/toni")
        menus.closeAll()

        menus.openFavorite("/home/toni/notas", Qt.point(10, 10))
        compare(menus.favoriteTargetPath, "/home/toni/notas")
        menus.closeAll()

        menus.openBookmark(2, "/home/toni/CODIGO", Qt.point(10, 10))
        compare(menus.bookmarkTargetPath, "/home/toni/CODIGO",
                "el marcador llegaba al menú sin su ruta")
        menus.closeAll()
    }

    // Un lugar virtual no tiene nada que enseñar: ni pestaña ni propiedades.
    function test_a_virtual_place_offers_neither() {
        menus.openPlace("TRASH", "Papelera", "", Qt.point(10, 10))
        compare(menus.placeTargetPath, "")
        menus.closeAll()
    }

    function test_the_device_menu_carries_the_mount_point() {
        menus.openDevice("USB", "/run/media/toni/USB", Qt.point(10, 10))
        compare(menus.deviceMountPoint, "/run/media/toni/USB")
        compare(menus.deviceCanOpenTab, true)
        menus.closeAll()

        menus.openDevice("USB sin montar", "", Qt.point(10, 10))
        compare(menus.deviceCanOpenTab, false,
                "ofrecería abrir una pestaña a ninguna parte")
        menus.closeAll()
    }
}
