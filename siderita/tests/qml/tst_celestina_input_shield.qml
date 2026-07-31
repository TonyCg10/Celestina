import QtQuick
import QtQuick.Controls
import QtTest 1.3
import org.celestina.siderita 1.0

// El contrato de las superficies flotantes: lo que se pinta sobre la lista no
// deja pasar puntero al contenido de debajo, y sigue dejando pasar la rueda.
// El "contenido" de aquí replica lo que hace un delegado de fila real: un
// MouseArea de tres botones con hover y un DragHandler que se lleva un agarre
// pasivo en la pulsación.
TestCase {
    id: testCase
    name: "CelestinaInputShield"
    width: 400
    height: 300
    visible: true
    when: windowShown

    property int contentClicks: 0
    property int contentRightClicks: 0
    property int contentMiddleClicks: 0
    property int contentWheels: 0
    property bool contentDragged: false
    property bool contentHovered: contentMouse.containsMouse
    property int surfaceClicks: 0

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
        onWheel: function(wheel) { testCase.contentWheels++ }

        DragHandler {
            id: contentDrag
            target: null
            dragThreshold: 8
            onActiveChanged: if (active) testCase.contentDragged = true
        }
    }

    // La superficie flotante: una caja con su propio control dentro, como una
    // pastilla con botón o una cabecera con acciones.
    Rectangle {
        id: surface
        x: 100
        y: 100
        width: 200
        height: 60
        color: "transparent"

        CelestinaInputShield { id: shield }

        Button {
            id: surfaceButton
            x: 140
            y: 4
            width: 50
            height: 26
            text: "Ok"
            onClicked: testCase.surfaceClicks++
        }

        TextField {
            id: surfaceField
            x: 5
            y: 4
            width: 120
            height: 26
            text: "seleccionable"
        }
    }

    function init() {
        contentClicks = 0
        contentRightClicks = 0
        contentMiddleClicks = 0
        contentWheels = 0
        contentDragged = false
        surfaceClicks = 0
        shield.active = true
        surfaceField.select(0, 0)
        mouseMove(testCase, 10, 10)
    }

    function test_swallows_every_button() {
        mouseClick(surface, 60, 48, Qt.LeftButton)
        mouseClick(surface, 60, 48, Qt.RightButton)
        mouseClick(surface, 60, 48, Qt.MiddleButton)
        compare(contentClicks, 0, "el clic izquierdo alcanzó el contenido")
        compare(contentRightClicks, 0, "el clic derecho alcanzó el contenido")
        compare(contentMiddleClicks, 0, "el clic central alcanzó el contenido")
    }

    function test_swallows_hover() {
        mouseMove(surface, 60, 48)
        verify(!contentHovered, "el contenido se iluminó bajo la superficie")
        mouseMove(testCase, 10, 250)
        verify(contentHovered, "el contenido dejó de responder fuera de la caja")
    }

    function test_swallows_drag() {
        mousePress(surface, 60, 48, Qt.LeftButton)
        mouseMove(surface, 90, 50)
        mouseMove(surface, 130, 52)
        mouseMove(surface, 170, 52)
        mouseRelease(surface, 170, 52, Qt.LeftButton)
        verify(!contentDragged, "el arrastre arrancó sobre el contenido")
    }

    function test_lets_the_wheel_through() {
        mouseWheel(surface, 60, 48, 0, -120)
        compare(contentWheels, 1, "la rueda dejó de desplazar el contenido")
    }

    function test_keeps_its_own_controls_alive() {
        mouseClick(surfaceButton, 25, 13, Qt.LeftButton)
        compare(surfaceClicks, 1, "el botón de la superficie dejó de responder")
        compare(contentClicks, 0, "el clic del botón llegó también al contenido")
    }

    // El escudo reclama el arrastre, pero un campo de texto de la propia
    // superficie está por encima: barrer para seleccionar debe seguir siendo
    // suyo.
    function test_keeps_text_selection() {
        mousePress(surfaceField, 10, 13, Qt.LeftButton)
        mouseMove(surfaceField, 40, 13)
        mouseMove(surfaceField, 80, 13)
        mouseRelease(surfaceField, 80, 13, Qt.LeftButton)
        verify(surfaceField.selectedText.length > 0,
               "la selección de texto dejó de funcionar dentro de la superficie")
        verify(!contentDragged, "el barrido de selección arrastró el contenido")
    }

    // Apagado explícito (una superficie que no está pintando) el contenido
    // vuelve a ser suyo: es lo que usa el fondo de un control. Es además la
    // fixture negativa de las tres comprobaciones de arriba — sin escudo, el
    // clic, el hover y el arrastre sí llegan a la fila de debajo.
    function test_inactive_shield_passes_through() {
        shield.active = false

        mouseClick(surface, 60, 48, Qt.LeftButton)
        compare(contentClicks, 1, "un escudo inactivo siguió bloqueando el clic")

        mouseMove(testCase, 10, 250)
        mouseMove(surface, 60, 48)
        verify(contentHovered, "un escudo inactivo siguió bloqueando el hover")

        mousePress(surface, 60, 48, Qt.LeftButton)
        mouseMove(surface, 90, 50)
        mouseMove(surface, 130, 52)
        mouseMove(surface, 170, 52)
        mouseRelease(surface, 170, 52, Qt.LeftButton)
        verify(contentDragged, "un escudo inactivo siguió bloqueando el arrastre")
    }
}
