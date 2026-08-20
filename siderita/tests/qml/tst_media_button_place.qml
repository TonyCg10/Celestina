import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// Where the phone's media button sits once the heading is gone: centred on the
// search glyph, one gap below the bar. It was positioned with mapToItem before,
// which is evaluated once and never again — the button ended up in the corner.
TestCase {
    id: testCase
    name: "MediaButtonPlace"
    width: 600
    height: 200
    visible: true
    when: windowShown

    Item {
        id: barra
        width: 400
        height: 44
        x: 60
        y: 30
        readonly property real searchCentreFromRight: 44 / 2
    }

    PhoneMediaButton {
        id: boton
        width: 32
        height: 32
        connected: true
        anchors.right: barra.right
        anchors.rightMargin: barra.searchCentreFromRight - width / 2
        anchors.top: barra.bottom
        anchors.topMargin: CelestinaTheme.spaceSm
    }

    function test_a_it_hangs_under_the_search_glyph() {
        waitForRendering(testCase)
        // Same centre as the glyph, which sits half a collapsed pill in.
        const glyphCentre = barra.x + barra.width - barra.searchCentreFromRight
        compare(boton.x + boton.width / 2, glyphCentre)
        // And a gap below the bar, never overlapping it.
        compare(boton.y, barra.y + barra.height + CelestinaTheme.spaceSm)
        verify(boton.y > barra.y + barra.height, "it overlapped the bar")
    }

    function test_b_it_follows_the_bar_when_it_moves() {
        barra.x = 120
        waitForRendering(testCase)
        const glyphCentre = barra.x + barra.width - barra.searchCentreFromRight
        compare(boton.x + boton.width / 2, glyphCentre,
                "the button stayed behind when the bar moved")
    }
}
