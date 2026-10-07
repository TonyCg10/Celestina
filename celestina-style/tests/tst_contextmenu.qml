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
                id: hiddenRowMenuComponent
                GlassContextMenu {
                    readonly property alias hiddenRow: hiddenRow
                    readonly property alias firstRow: firstRow
                    readonly property alias lastRow: lastRow
                    backdropSource: stage
                    GlassMenuItem { id: firstRow; text: "one" }
                    GlassMenuItem { id: hiddenRow; visible: false; text: "two" }
                    GlassMenuItem { id: lastRow; text: "three" }
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

    // A window tall enough for a menu built like Fluorita's stream menu: two
    // section headers, each followed by rows an Instantiator inserts after it,
    // plus a hidden header and a hidden row. Its rows arrive after the open,
    // so its real height is only known a frame later.
    Window {
        id: tallWindow

        width: 900
        height: 600
        visible: true

        Item {
            id: tallStage
            anchors.fill: parent

            Item {
                id: streamBar
                x: 0; y: 540; width: 900; height: 60
                Button { id: streamButton; x: 820; y: 14; width: 32; height: 32; text: "s" }
            }

            Component {
                id: streamMenuComponent
                GlassContextMenu {
                    id: streamMenu

                    property bool timed: false
                    property bool choosableAudio: false
                    property bool choosableSubtitles: false
                    readonly property alias audioHeader: audioHeader
                    readonly property alias subtitleHeader: subtitleHeader
                    readonly property alias noSubtitles: noSubtitles
                    readonly property alias endHeader: endHeader
                    readonly property alias speedHeader: speedHeader

                    function slotAfter(anchor, offset) {
                        for (let i = 0; i < streamMenu.count; ++i) {
                            if (streamMenu.itemAt(i) === anchor)
                                return i + 1 + offset
                        }
                        return streamMenu.count
                    }

                    backdropSource: tallStage

                    GlassMenuSection { id: audioHeader; visible: streamMenu.choosableAudio; text: "Audio" }
                    GlassMenuSection { id: subtitleHeader; visible: streamMenu.choosableSubtitles; text: "Subtitles" }
                    GlassMenuItem {
                        id: noSubtitles
                        visible: streamMenu.choosableSubtitles
                        implicitHeight: visible ? CelestinaTheme.controlHeight : 0
                        text: "No subtitles"
                    }
                    GlassMenuSection { id: endHeader; visible: streamMenu.timed; text: "When done" }
                    GlassMenuSection { id: speedHeader; visible: streamMenu.timed; text: "Speed" }

                    Instantiator {
                        model: streamMenu.choosableAudio ? ["a1", "a2"] : []
                        delegate: GlassMenuItem { required property string modelData; text: modelData }
                        onObjectAdded: (i, o) => streamMenu.insertItem(streamMenu.slotAfter(audioHeader, i), o)
                        onObjectRemoved: (i, o) => streamMenu.removeItem(o)
                    }
                    Instantiator {
                        model: streamMenu.choosableSubtitles ? ["s1"] : []
                        delegate: GlassMenuItem { required property string modelData; text: modelData }
                        onObjectAdded: (i, o) => streamMenu.insertItem(streamMenu.slotAfter(noSubtitles, i), o)
                        onObjectRemoved: (i, o) => streamMenu.removeItem(o)
                    }
                    Instantiator {
                        model: streamMenu.timed ? ["stop", "next", "repeat"] : []
                        delegate: GlassMenuItem { required property string modelData; text: modelData }
                        onObjectAdded: (i, o) => streamMenu.insertItem(streamMenu.slotAfter(endHeader, i), o)
                        onObjectRemoved: (i, o) => streamMenu.removeItem(o)
                    }
                    Instantiator {
                        model: streamMenu.timed ? ["0.5", "0.75", "1", "1.25", "1.5", "2"] : []
                        delegate: GlassMenuItem { required property string modelData; text: modelData }
                        onObjectAdded: (i, o) => streamMenu.insertItem(streamMenu.slotAfter(speedHeader, i), o)
                        onObjectRemoved: (i, o) => streamMenu.removeItem(o)
                    }
                }
            }
        }
    }

    function test_sections_keep_their_groups_and_menu_clears_button() {
        const menu = createTemporaryObject(streamMenuComponent, streamBar)
        verify(menu)
        const opened = createTemporaryObject(openedSpy, testCase, { target: menu })
        // The model changes right before the open, as a stream menu's does.
        menu.timed = true
        menu.choosableAudio = true
        menu.popupBeside(streamButton, true)
        opened.wait()

        const box = streamButton.mapToItem(null, 0, 0)
        const bottom = function() {
            return menu.background.mapToItem(null, 0, 0).y + menu.background.height
        }
        tryVerify(function() { return menu.background.height > 300 }, 1000,
                  "the menu grew to its real height")
        tryVerify(function() { return bottom() <= box.y - CelestinaTheme.spaceSm + 0.5 }, 1000,
                  "menu bottom " + bottom() + " vs button top " + box.y)
        verify(menu.background.mapToItem(null, 0, 0).y >= menu.margins - 0.5)

        // Content order: each header directly before its own rows.
        const texts = []
        for (let i = 0; i < menu.count; ++i)
            texts.push(menu.itemAt(i).text)
        compare(texts.join("|"),
                "Audio|a1|a2|Subtitles|No subtitles|When done|stop|next|repeat"
                + "|Speed|0.5|0.75|1|1.25|1.5|2")

        // Hidden rows take no room.
        compare(menu.subtitleHeader.height, 0)
        compare(menu.noSubtitles.height, 0)
        verify(menu.audioHeader.height > 0)

        // On screen: every visible row starts where the previous one ends,
        // so a header sits right above the first row of its group.
        let previous = null
        for (let i = 0; i < menu.count; ++i) {
            const item = menu.itemAt(i)
            if (!item.visible)
                continue
            const y = item.mapToItem(null, 0, 0).y
            if (previous)
                fuzzyCompare(y, previous.y + previous.h, 0.5, item.text + " follows the row above")
            previous = { y: y, h: item.height }
        }
        const audioBottom = menu.audioHeader.mapToItem(null, 0, 0).y + menu.audioHeader.height
        fuzzyCompare(menu.itemAt(1).mapToItem(null, 0, 0).y, audioBottom, 0.5)
        menu.close()
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

    function test_hidden_item_takes_no_room() {
        const r = openBeside(topButton, false, stage, hiddenRowMenuComponent)
        compare(r.menu.hiddenRow.height, 0)
        verify(r.menu.firstRow.height > 0)
        // The menu's ListView lays the rows out again a frame after they
        // become visible on the open.
        const gap = function() {
            return r.menu.lastRow.mapToItem(null, 0, 0).y
                - (r.menu.firstRow.mapToItem(null, 0, 0).y + r.menu.firstRow.height)
        }
        tryVerify(function() { return Math.abs(gap()) < 0.5 }, 1000,
                  "the row after the hidden one sits right under the visible one, gap " + gap())
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
