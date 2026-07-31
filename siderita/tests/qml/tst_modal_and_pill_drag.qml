import QtQuick
import QtQuick.Controls
import QtTest 1.3
import org.celestina.siderita 1.0

// El caso que se escapó de la primera pasada: una caja flotante puede tragarse
// el clic y aun así dejar pasar el *arrastre*. Aquí la lista de debajo arrastra
// como FolderRowDelegate (umbral 8), y encima van la capa modal compartida con
// su GlassCard —como la usa PropertiesDialog— y una pastilla flotante. Barrer
// sobre cualquiera de las dos movía el archivo tapado.
TestCase {
    id: tc
    name: "ModalAndPillDrag"
    width: 700
    height: 500
    visible: true
    when: windowShown

    property bool dragStarted: false
    property string draggedName: ""
    property int cardClicks: 0

    Item {
        id: ghost
        width: 10
        height: 10
        Drag.active: false
        Drag.dragType: Drag.Automatic
    }

    function startEntryDrag(name, handler) {
        tc.dragStarted = true
        tc.draggedName = name
    }

    ListView {
        id: list
        anchors.fill: parent
        model: 12
        clip: true

        delegate: Item {
            id: rowRoot
            required property int index
            width: list.width
            height: 40

            Rectangle {
                anchors.fill: parent
                color: pointer.containsMouse ? "#333" : "#111"
            }

            MouseArea {
                id: pointer
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                hoverEnabled: true
            }

            DragHandler {
                id: rowDrag
                target: null
                dragThreshold: 8
                onActiveChanged: {
                    if (active)
                        tc.startEntryDrag("fila-" + rowRoot.index, rowDrag)
                }
            }
        }
    }

    // ── La capa modal compartida, como PropertiesDialog la usa ─────────────
    CelestinaModalLayer {
        id: modal
        anchors.fill: parent
        z: 68
        shown: false

        GlassCard {
            id: card
            anchors.centerIn: parent
            width: 400
            height: 300
            backdropSource: list

            MouseArea { anchors.fill: parent }

            TextField {
                id: cardField
                x: 20
                y: 20
                width: 200
                height: 30
                text: "texto seleccionable"
            }

            Button {
                id: cardButton
                x: 250
                y: 20
                width: 100
                height: 30
                text: "Aceptar"
                onClicked: tc.cardClicks++
            }
        }
    }

    InfoPill {
        id: pill
        x: 40
        y: 420
        z: 20
        backdrop: null
        text: "una pastilla flotante"
    }

    function init() {
        dragStarted = false
        draggedName = ""
        modal.shown = false
        cardClicks = 0
        cardField.select(0, 0)
        wait(300)
        mouseMove(tc, 5, 5)
    }

    function test_a_modal_card_empty_area_drag() {
        modal.shown = true
        wait(60)
        // Zona vacía de la tarjeta, lejos de cualquier control.
        mousePress(card, 200, 200, Qt.LeftButton)
        mouseMove(card, 210, 206)
        mouseMove(card, 240, 220)
        mouseMove(card, 300, 250)
        mouseRelease(card, 300, 250, Qt.LeftButton)
        verify(!dragStarted, "el arrastre en la tarjeta movió: " + draggedName)
    }

    function test_b_pill_drag() {
        mousePress(pill, 30, 15, Qt.LeftButton)
        mouseMove(pill, 40, 21)
        mouseMove(pill, 70, 35)
        mouseMove(pill, 130, 60)
        mouseRelease(pill, 130, 60, Qt.LeftButton)
        verify(!dragStarted, "el arrastre en la pastilla movió: " + draggedName)
    }

    // Los controles de la tarjeta conservan lo suyo con el modal abierto.
    function test_ba_card_controls_stay_alive() {
        modal.shown = true
        wait(60)
        mouseClick(cardButton, 50, 15, Qt.LeftButton)
        compare(cardClicks, 1, "el botón del diálogo dejó de responder")

        mousePress(cardField, 10, 15, Qt.LeftButton)
        mouseMove(cardField, 60, 15)
        mouseMove(cardField, 120, 15)
        mouseRelease(cardField, 120, 15, Qt.LeftButton)
        verify(cardField.selectedText.length > 0,
               "la selección de texto del diálogo dejó de funcionar")
        verify(!dragStarted, "seleccionar texto arrastró una fila")
    }

    // El barrido que *sale* de la caja es la forma que se escapaba: mientras el
    // puntero seguía dentro, el MouseArea del escudo retenía el agarre; camino
    // de la lista, el handler de la fila se lo llevaba.
    function test_bb_sweep_leaving_the_pill() {
        mousePress(pill, 20, 15, Qt.LeftButton)
        mouseMove(pill, 30, 21)
        mouseMove(pill, 60, 40)
        mouseMove(pill, 140, 90)
        mouseMove(pill, 240, 140)
        mouseRelease(pill, 240, 140, Qt.LeftButton)
        verify(!dragStarted, "salir de la pastilla arrastró: " + draggedName)
    }

    function test_bc_sweep_leaving_the_modal_card() {
        modal.shown = true
        wait(60)
        mousePress(card, 320, 240, Qt.LeftButton)
        mouseMove(card, 330, 246)
        mouseMove(card, 360, 270)
        mouseMove(card, 430, 330)
        mouseRelease(card, 430, 330, Qt.LeftButton)
        verify(!dragStarted, "salir de la tarjeta arrastró: " + draggedName)
    }

    // Fixture negativa: en la lista desnuda el arrastre sí debe arrancar.
    function test_c_bare_list_still_drags() {
        mousePress(list, 300, 300, Qt.LeftButton)
        mouseMove(list, 310, 306)
        mouseMove(list, 340, 320)
        mouseRelease(list, 340, 320, Qt.LeftButton)
        verify(dragStarted, "la lista dejó de poder arrastrar")
    }
}
