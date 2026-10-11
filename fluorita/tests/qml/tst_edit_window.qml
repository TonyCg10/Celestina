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

    function openWindow() {
        const win = createTemporaryObject(windowComponent, testCase, { key: testCase.picture })
        verify(win)
        tryVerify(function() { return win.visible })
        tryVerify(function() { return win.editor.open })
        return win
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

    Component {
        id: signalSpy
        SignalSpy {}
    }
}
