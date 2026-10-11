import QtQuick
import QtTest 1.3
import org.celestina.fluorita 1.0

// The floating edit window over the fixture picture, with the stand-in
// editor: its title, the picture on the surface, the two ways a close goes
// (at once when nothing changed, through the three choices when something
// did), the two save outcomes by their names, and the saved result offered
// to other programs as a copy.
TestCase {
    id: testCase
    name: "EditWindow"
    when: windowShown

    // The fixture's path, which is also its path key: every byte in it is one
    // a key keeps as it is.
    readonly property string picture: decodeURIComponent(
        Qt.resolvedUrl("../fixtures/picture.png").toString().substring("file://".length))

    Component {
        id: windowComponent
        EditWindow {}
    }

    function findNamed(node, name) {
        if (node.objectName === name)
            return node
        const kids = node.children !== undefined ? node.children : []
        let found = null
        let i = 0
        while (found === null && i < kids.length) {
            found = findNamed(kids[i], name)
            i += 1
        }
        return found
    }

    // Every visible button whose text or help text is `label`; a button's own
    // label item carries the text too, and is not a button.
    function buttonsLabelled(node, label, out) {
        if (node.visible && node.clicked !== undefined
                && (node.text === label || node.helpText === label))
            out.push(node)
        const kids = node.children !== undefined ? node.children : []
        let i = 0
        while (i < kids.length) {
            buttonsLabelled(kids[i], label, out)
            i += 1
        }
        return out
    }

    // The one visible button under `root` that says `label`.
    function button(root, label) {
        const found = buttonsLabelled(root, label, [])
        compare(found.length, 1, "one button says " + label)
        return found[0]
    }

    // The close question, once it is up.
    function question(win) {
        const found = findNamed(win.contentItem, "closeQuestion")
        tryVerify(function() { return found.shown })
        tryCompare(found, "opacity", 1)
        return found
    }

    // On a screen with room for the whole toolbar: the offscreen one is
    // smaller than any the editor opens on.
    function openWindow() {
        return openOn(2560, 1440)
    }

    function test_the_title_names_the_file_after_an_em_dash() {
        const win = openWindow()
        compare(win.title, "Editar — picture.png")
        compare(win.video, false)
        compare(win.editor.opened, [testCase.picture])
    }

    function test_the_surface_shows_the_picture() {
        const win = openWindow()
        const shown = findNamed(win.contentItem, "editPicture")
        verify(shown)
        compare(shown.source.toString(), "file://" + testCase.picture)
        tryCompare(shown, "status", Image.Ready)
        verify(shown.width > 0 && shown.height > 0)
    }

    function test_the_toolbar_names_both_outcomes() {
        const win = openWindow()
        win.editor.rotate(true)
        button(win.contentItem, "Guardar ambas")
        button(win.contentItem, "Guardar solo la editada")
    }

    function test_an_unchanged_close_closes_without_a_question() {
        const win = openWindow()
        const finished = signalSpy.createObject(testCase, { target: win, signalName: "finished" })
        win.close()
        tryVerify(function() { return !win.visible })
        compare(finished.count, 1)
        compare(win.editor.saves, [])
        verify(!findNamed(win.contentItem, "closeQuestion").shown)
    }

    function test_a_changed_close_asks_the_three_choices() {
        const win = openWindow()
        win.editor.rotate(true)
        win.close()
        const asked = question(win)
        verify(win.visible, "the window waits for the answer")
        const keep = button(asked, "Guardar ambas")
        compare(keep.role, CelestinaButton.Primary)
        button(asked, "Guardar solo la editada")
        button(asked, "Descartar")
        compare(win.editor.saves, [])
    }

    function test_keep_both_saves_a_copy_beside_and_closes() {
        const win = openWindow()
        win.editor.rotate(true)
        win.close()
        mouseClick(button(question(win), "Guardar ambas"))
        compare(win.editor.saves, [false])
        tryVerify(function() { return !win.visible })
        compare(win.editor.savedKey, testCase.picture.replace(".png", " (editado).png"))
    }

    function test_only_the_edited_replaces_and_closes() {
        const win = openWindow()
        win.editor.rotate(true)
        win.close()
        mouseClick(button(question(win), "Guardar solo la editada"))
        compare(win.editor.saves, [true])
        tryVerify(function() { return !win.visible })
    }

    function test_discard_closes_and_saves_nothing() {
        const win = openWindow()
        win.editor.rotate(true)
        win.close()
        mouseClick(button(question(win), "Descartar"))
        tryVerify(function() { return !win.visible })
        compare(win.editor.saves, [])
    }

    function test_a_failed_save_on_the_way_out_keeps_the_window_and_the_edit() {
        const win = openWindow()
        const finished = signalSpy.createObject(testCase, { target: win, signalName: "finished" })
        win.editor.rotate(true)
        win.editor.failNextSave = true
        win.close()
        mouseClick(button(question(win), "Guardar ambas"))
        compare(win.editor.saves, [false])
        tryVerify(function() { return !win.editor.saving })
        // The answer settles a turn later; the window must still be there.
        wait(50)
        verify(win.visible, "a failed save does not close the window")
        verify(win.editor.open && win.editor.edited, "the edit is kept")
        compare(finished.count, 0)
        // Leaving again asks again, and «Descartar» closes once: the edit
        // ends once, and the window once.
        const ended = signalSpy.createObject(testCase, {
            target: findNamed(win.contentItem, "editSurface"), signalName: "closed" })
        win.close()
        mouseClick(button(question(win), "Descartar"))
        tryVerify(function() { return !win.visible })
        wait(50)
        compare(ended.count, 1)
        compare(finished.count, 1)
        compare(win.editor.saves, [false])
    }

    function test_staying_keeps_the_edit() {
        const win = openWindow()
        win.editor.rotate(true)
        win.close()
        const asked = question(win)
        asked.dismissRequested()
        tryVerify(function() { return !asked.shown })
        verify(win.visible)
        verify(win.editor.open && win.editor.edited)
    }

    function test_a_saved_result_drags_out_as_a_copy() {
        const win = openWindow()
        win.editor.rotate(true)
        mouseClick(button(win.contentItem, "Guardar ambas"))
        tryVerify(function() { return win.editor.savedKey.length > 0 })
        verify(win.visible, "a save from the toolbar keeps the window")
        const result = findNamed(win.contentItem, "savedResult")
        verify(result)
        tryVerify(function() { return result.visible })
        compare(result.Drag.dragType, Drag.Automatic)
        compare(result.Drag.supportedActions, Qt.CopyAction)
        compare(result.Drag.proposedAction, Qt.CopyAction)
        compare(result.Drag.mimeData["text/uri-list"], win.editor.savedUrl + "\r\n")
        // Nothing is left to ask about once the result is on disk.
        win.close()
        tryVerify(function() { return !win.visible })
    }

    // The window's size, against the bar along its bottom (FLU-P2-A). The
    // offscreen screen is small, so each window is given the room of the
    // screen it pretends to be on.
    function openOn(screenWidth, screenHeight) {
        const win = createTemporaryObject(windowComponent, testCase, {
            key: testCase.picture,
            availableWidth: screenWidth,
            availableHeight: screenHeight
        })
        verify(win)
        tryVerify(function() { return win.visible })
        tryVerify(function() { return win.editor.open })
        // The content follows the window's size a turn after it is shown.
        tryCompare(win.contentItem, "width", win.width)
        return win
    }

    function toolbarOf(win) {
        const toolbar = findNamed(win.contentItem, "editToolbar")
        verify(toolbar)
        verify(toolbar.implicitWidth > 0)
        return toolbar
    }

    function test_the_window_opens_with_the_whole_toolbar_in_it() {
        const win = openOn(2560, 1440)
        const toolbar = toolbarOf(win)
        const whole = Math.ceil(toolbar.implicitWidth + CelestinaTheme.spaceLg * 2)
        verify(win.width >= whole, "the window " + win.width + " holds the bar " + whole)
        verify(win.width >= 1280)
        verify(win.height >= 800)
        compare(win.minimumWidth, whole)
        compare(toolbar.width, toolbar.implicitWidth)
        verify(!toolbar.cut)
        verify(!findNamed(toolbar, "toolbarStrip").interactive)
        // Every action is there to be seen: the last one ends inside the
        // window.
        const last = button(toolbar, "Cerrar")
        const end = last.mapToItem(win.contentItem, last.width, 0).x
        verify(end <= win.width, "«Cerrar» ends at " + end + " of " + win.width)
    }

    function test_the_window_takes_at_most_nine_tenths_of_the_screen() {
        const win = openOn(1300, 860)
        compare(win.width, 1170)
        compare(win.height, 774)
        verify(win.width >= toolbarOf(win).implicitWidth + CelestinaTheme.spaceLg * 2)
    }

    // The refusal itself is the compositor's, told through the window's
    // minimum size (the offscreen platform ignores size hints, and takes a
    // program's resize as given). What Qt does with the minimum is shown
    // here: a window narrower than it is widened the moment it applies.
    function test_narrower_than_the_toolbar_is_refused() {
        const win = openOn(2560, 1440)
        const whole = Math.ceil(toolbarOf(win).implicitWidth + CelestinaTheme.spaceLg * 2)
        compare(win.minimumWidth, whole)
        // A screen too narrow lowers the minimum, and the window is made
        // narrower than the bar...
        win.availableWidth = Math.floor((whole - 200) / 0.9)
        verify(win.minimumWidth < whole)
        win.width = win.minimumWidth
        tryCompare(win, "width", win.minimumWidth)
        // ...then the whole bar's minimum returns and the window is widened.
        win.availableWidth = 2560
        compare(win.minimumWidth, whole)
        tryVerify(function() { return win.width >= whole },
                  1000, "the window " + win.width + " is never narrower than " + whole)
        tryVerify(function() { return !toolbarOf(win).cut })
    }

    function test_a_screen_narrower_than_the_toolbar_scrolls_it() {
        const measured = openOn(2560, 1440)
        const whole = Math.ceil(toolbarOf(measured).implicitWidth + CelestinaTheme.spaceLg * 2)
        // A screen whose nine tenths fall short of the bar.
        const win = openOn(Math.floor(whole * 0.8), 1440)
        const toolbar = toolbarOf(win)
        const strip = findNamed(toolbar, "toolbarStrip")
        compare(win.minimumWidth, win.largestWidth)
        verify(win.width < whole)
        tryVerify(function() { return toolbar.cut })
        verify(toolbar.width <= win.width - CelestinaTheme.spaceLg * 2)
        verify(strip.interactive && strip.clip)
        compare(strip.contentX, 0)
        // The wheel moves the tools sideways...
        mouseWheel(strip, strip.width / 2, strip.height / 2, 0, -120)
        tryVerify(function() { return strip.contentX > 0 })
        // ...and Tab to a tool out of sight brings it in.
        strip.contentX = 0
        const last = button(toolbar, "Cerrar")
        last.forceActiveFocus()
        tryVerify(function() {
            const at = last.mapToItem(strip, 0, 0).x
            return at >= 0 && at + last.width <= strip.width + 0.5
        })
        // Every action is still pressed where it is shown.
        mouseClick(last)
        tryVerify(function() { return !win.visible })
    }

    // The focus ring shown under `node`: the visible outline of a tool that
    // has visual focus.
    function shownRing(node) {
        if (node.visible && node.shown === true && node.target !== undefined
                && node.cornerRadius !== undefined)
            return node
        const kids = node.children !== undefined ? node.children : []
        let found = null
        let i = 0
        while (found === null && i < kids.length) {
            found = shownRing(kids[i])
            i += 1
        }
        return found
    }

    // Reached by Tab in a cut toolbar, the first and the last tool show
    // their whole ring inside the clipped strip.
    function test_a_focused_tool_in_a_cut_toolbar_shows_its_whole_ring() {
        const measured = openOn(2560, 1440)
        const whole = Math.ceil(toolbarOf(measured).implicitWidth + CelestinaTheme.spaceLg * 2)
        const win = openOn(Math.floor(whole * 0.8), 1440)
        const toolbar = toolbarOf(win)
        const strip = findNamed(toolbar, "toolbarStrip")
        tryVerify(function() { return toolbar.cut && strip.clip })
        win.requestActivate()
        tryVerify(function() { return win.active })
        const ends = [button(toolbar, "Girar a la izquierda"), button(toolbar, "Cerrar")]
        let i = 0
        while (i < ends.length) {
            const tool = ends[i]
            tool.forceActiveFocus(Qt.TabFocusReason)
            let ring = null
            tryVerify(function() { ring = shownRing(tool); return ring !== null },
                      1000, "a ring on " + tool.helpText)
            tryVerify(function() {
                const at = ring.mapToItem(strip, 0, 0).x
                return at >= -0.01 && at + ring.width <= strip.width + 0.01
            }, 1000, "the ring of " + tool.helpText + " lies inside the strip")
            i += 1
        }
    }

    Component {
        id: signalSpy
        SignalSpy {}
    }
}
