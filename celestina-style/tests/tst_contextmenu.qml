import QtQuick
import QtQuick.Controls
import QtQuick.Window
import QtTest
import CelestinaStyle

// A menu opened from a button must sit beside the button, never over it. The
// menu's own `height` is 0 until its first open, so the placement has to come
// from `implicitHeight`; these cases open a fresh menu each time to prove it.
TestCase {
    id: testCase

    name: "GlassContextMenu"
    when: testWindow.visible

    Window {
        id: testWindow

        width: 480
        height: 320
        visible: true

        Item {
            id: stage
            anchors.fill: parent

            Button { id: topButton; x: 12; y: 12; width: 80; height: 32; text: "top" }
            Button { id: bottomButton; x: 380; y: 276; width: 80; height: 32; text: "bottom" }

            Item {
                id: bottomBar
                x: 200; y: 280; width: 120; height: 40
                Button { id: barButton; width: 80; height: 32; text: "bar" }
            }
            Item {
                id: midBar
                x: 200; y: 100; width: 120; height: 40
                Button { id: midButton; width: 80; height: 32; text: "mid" }
            }
            Item {
                id: narrowBar
                x: 400; y: 100; width: 40; height: 40
                Button { id: narrowButton; width: 40; height: 32; text: "n" }
            }

            Component {
                id: menuComponent
                GlassContextMenu {
                    backdropSource: stage
                    GlassMenuItem { text: "one" }
                    GlassMenuItem { text: "two" }
                    GlassMenuItem { text: "three" }
                }
            }
            Component {
                id: repeaterMenuComponent
                GlassContextMenu {
                    property int rows: 1
                    backdropSource: stage
                    Repeater {
                        model: parent.rows
                        delegate: GlassMenuItem { required property int index; text: "row " + index }
                    }
                }
            }
        }
    }

    function init() {
        CelestinaTheme.reducedMotion = true
    }

    function openBeside(button, preferAbove, host, comp, rows) {
        const menu = createTemporaryObject(comp || menuComponent, host || stage)
        verify(menu)
        const opened = createTemporaryObject(openedSpy, testCase, { target: menu })
        if (rows !== undefined)
            menu.rows = rows
        menu.popupBeside(button, preferAbove)
        opened.wait()
        verify(menu.visible)
        const bg = menu.background.mapToItem(null, 0, 0)
        return { menu: menu, x: bg.x, y: bg.y, w: menu.background.width, h: menu.background.height,
                 box: button.mapToItem(null, 0, 0) }
    }

    function inside(r) {
        const m = r.menu.margins
        return r.x >= m - 0.5 && r.y >= m - 0.5
            && r.x + r.w <= testWindow.width - m + 0.5
            && r.y + r.h <= testWindow.height - m + 0.5
    }

    function test_nested_bar_opens_above() {
        for (const prefer of [true, false]) {
            const r = openBeside(barButton, prefer, bottomBar)
            verify(inside(r), "inside window, prefer=" + prefer)
            verify(r.y + r.h <= r.box.y - CelestinaTheme.spaceSm + 0.5, "above, prefer=" + prefer)
            verify(!overlaps(r, barButton, r.box))
            r.menu.close()
        }
    }

    function test_nested_mid_button_opens_below() {
        const r = openBeside(midButton, false, midBar)
        verify(r.y >= r.box.y + midButton.height + CelestinaTheme.spaceSm - 0.5, "below")
        verify(inside(r))
        r.menu.close()
    }

    function test_menu_wider_than_parent_stays_in_window() {
        const r = openBeside(narrowButton, false, narrowBar)
        verify(r.w > narrowBar.width)
        verify(inside(r), "x=" + r.x + " w=" + r.w)
        r.menu.close()
    }

    function test_menu_content_changed_right_before_open() {
        const r = openBeside(bottomButton, true, stage, repeaterMenuComponent, 6)
        verify(r.y + r.h <= r.box.y - CelestinaTheme.spaceSm + 0.5,
               "bottom " + (r.y + r.h) + " vs button top " + r.box.y)
        verify(!overlaps(r, bottomButton, r.box))
        r.menu.close()
    }

    Component {
        id: openedSpy
        SignalSpy { signalName: "opened" }
    }

    function overlaps(r, button, box) {
        return r.x < box.x + button.width && r.x + r.w > box.x
            && r.y < box.y + button.height && r.y + r.h > box.y
    }

    function test_below_the_button() {
        const r = openBeside(topButton, false)
        verify(r.y >= r.box.y + topButton.height + CelestinaTheme.spaceSm - 0.5,
               "menu top " + r.y + " must clear the button bottom plus the gap")
        verify(!overlaps(r, topButton, r.box))
        r.menu.close()
    }

    function test_above_on_first_open() {
        const r = openBeside(bottomButton, true)
        verify(r.y + r.h <= r.box.y - CelestinaTheme.spaceSm + 0.5,
               "menu bottom " + (r.y + r.h) + " must clear the button top minus the gap")
        verify(!overlaps(r, bottomButton, r.box))
        r.menu.close()
    }

    function test_flips_above_when_below_does_not_fit() {
        const r = openBeside(bottomButton, false)
        verify(r.y + r.h <= r.box.y - CelestinaTheme.spaceSm + 0.5)
        verify(!overlaps(r, bottomButton, r.box))
        r.menu.close()
    }

    function test_stays_inside_the_window_horizontally() {
        const r = openBeside(bottomButton, false)
        verify(r.x >= 0)
        verify(r.x + r.w <= testWindow.width + 0.5)
        r.menu.close()
    }
}
