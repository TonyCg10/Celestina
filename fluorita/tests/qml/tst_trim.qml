import QtQuick
import QtTest 1.3
import org.celestina.fluorita 1.0

// The video trim, over the stand-in trim, player and video surface: the
// bar's two handles (the start never passes the end, the keyboard reaches
// both), a handle that moves seeks the picture, play keeps to the span, a
// whole span saves nothing, the two outcomes reach `saveTrim`, a save in
// flight shows its progress and can be cancelled, and leaving with a span
// chosen asks first.
TestCase {
    id: testCase
    name: "Trim"
    when: windowShown
    width: 640
    height: 200
    // A test case is invisible by default, and so is a bar created inside
    // it: the pointer reaches only what is shown.
    visible: true

    readonly property string film: decodeURIComponent(
        Qt.resolvedUrl("../fixtures/three-seconds.mp4").toString().substring("file://".length))

    // A bar on its own, held by a host that takes every move as the trim
    // would: the bar itself never changes its values.
    Component {
        id: barHost

        Item {
            id: host

            property real start: 0
            property real end: 3
            property var moves: []

            width: 600
            height: 60

            readonly property TrimBar bar: trimBar

            TrimBar {
                id: trimBar

                anchors.fill: parent
                duration: 3
                start: host.start
                end: host.end
                minimumGap: 0.1
                onStartMoved: function(seconds) {
                    host.moves = host.moves.concat([["start", seconds]])
                    host.start = seconds
                }
                onEndMoved: function(seconds) {
                    host.moves = host.moves.concat([["end", seconds]])
                    host.end = seconds
                }
            }
        }
    }

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

    function button(root, label) {
        const found = buttonsLabelled(root, label, [])
        compare(found.length, 1, "one button says " + label)
        return found[0]
    }

    function openBar() {
        const host = createTemporaryObject(barHost, testCase)
        verify(host)
        return host
    }

    function openFilm() {
        const win = createTemporaryObject(windowComponent, testCase, { key: testCase.film })
        verify(win)
        tryVerify(function() { return win.visible })
        tryVerify(function() { return win.video })
        const surface = findNamed(win.contentItem, "trimSurface")
        verify(surface)
        tryVerify(function() { return surface.visible })
        tryCompare(surface.trim, "lengthSeconds", 3)
        // Keys go to the focused window: this one, as a person's would.
        win.requestActivate()
        tryVerify(function() { return win.active })
        return win
    }

    // The close question, once it is up and opaque.
    function question(surface) {
        const found = findNamed(surface, "trimCloseQuestion")
        tryVerify(function() { return found.shown })
        tryCompare(found, "opacity", 1)
        return found
    }

    function test_the_bar_has_two_named_handles() {
        const host = openBar()
        const first = findNamed(host, "trimStart")
        const last = findNamed(host, "trimEnd")
        verify(first && last)
        compare(first.Accessible.role, Accessible.Slider)
        compare(first.Accessible.name, "Inicio del recorte")
        compare(last.Accessible.name, "Final del recorte")
        // Each handle tells the time it stands on.
        verify(first.Accessible.description.startsWith("0:00,000"), first.Accessible.description)
        verify(last.Accessible.description.startsWith("0:03,000"), last.Accessible.description)
        host.start = 1.5
        verify(first.Accessible.description.startsWith("0:01,500"), first.Accessible.description)
        verify(first.activeFocusOnTab && last.activeFocusOnTab)
        verify(last.x > first.x)
    }

    function test_the_start_cannot_pass_the_end_from_the_keyboard() {
        const host = openBar()
        host.end = 2
        const first = findNamed(host, "trimStart")
        first.forceActiveFocus()
        keyClick(Qt.Key_End)
        fuzzyCompare(host.start, 1.9, 0.0001)
        keyClick(Qt.Key_Right)
        fuzzyCompare(host.start, 1.9, 0.0001)
        keyClick(Qt.Key_Home)
        compare(host.start, 0)
        keyClick(Qt.Key_Right)
        fuzzyCompare(host.start, 1 / 30, 0.0001)
        keyClick(Qt.Key_Right, Qt.ShiftModifier)
        fuzzyCompare(host.start, 1 / 30 + 1, 0.0001)

        const last = findNamed(host, "trimEnd")
        last.forceActiveFocus()
        keyClick(Qt.Key_Home)
        fuzzyCompare(host.end, host.start + 0.1, 0.0001)
        keyClick(Qt.Key_End)
        compare(host.end, 3)
    }

    function test_the_start_cannot_be_dragged_past_the_end() {
        const host = openBar()
        host.end = 1.5
        const first = findNamed(host, "trimStart")
        const at = first.mapToItem(host, first.width / 2, first.height / 2)
        mouseDrag(host, at.x, at.y, host.width, 0)
        verify(host.moves.length > 0)
        fuzzyCompare(host.start, 1.4, 0.0001)
        compare(host.end, 1.5)

        const last = findNamed(host, "trimEnd")
        const from = last.mapToItem(host, last.width / 2, last.height / 2)
        mouseDrag(host, from.x, from.y, -host.width, 0)
        fuzzyCompare(host.end, 1.5, 0.0001)
    }

    function test_a_video_key_opens_the_trim_not_the_picture_editor() {
        const win = openFilm()
        compare(win.title, "Editar — three-seconds.mp4")
        compare(win.editor.open, false)
        const surface = findNamed(win.contentItem, "trimSurface")
        compare(surface.trim.opened, [testCase.film])
        compare(surface.player.opened, [testCase.film])
        verify(!findNamed(win.contentItem, "editSurface").visible)
        // A film being trimmed is not announced as what the desktop plays.
        compare(surface.player.announced, false)
        // An MP4 keeps its container: nothing to say.
        verify(!findNamed(surface, "trimContainerNotice").visible)
    }

    function test_the_arrows_step_one_frame_of_the_film() {
        const win = openFilm()
        const surface = findNamed(win.contentItem, "trimSurface")
        // The film's description says 60 frames a second.
        surface.trim.minimumSeconds = 1 / 60
        const first = findNamed(surface, "trimStart")
        first.forceActiveFocus()
        keyClick(Qt.Key_Right)
        fuzzyCompare(surface.trim.trimStart, 1 / 60, 0.00001)
        const last = findNamed(surface, "trimEnd")
        last.forceActiveFocus()
        keyClick(Qt.Key_Home)
        fuzzyCompare(surface.trim.trimEnd, 1 / 60 + 1 / 60, 0.00001)
    }

    function test_what_a_trim_leaves_out_is_said_before_saving() {
        const win = openFilm()
        const surface = findNamed(win.contentItem, "trimSurface")
        surface.trim.containerNotice = "container changes, one stream left out"
        const notice = findNamed(surface, "trimContainerNotice")
        verify(notice.visible)
        compare(notice.text, surface.trim.containerNotice)
    }

    function test_moving_a_handle_seeks_the_picture() {
        const win = openFilm()
        const surface = findNamed(win.contentItem, "trimSurface")
        const first = findNamed(surface, "trimStart")
        first.forceActiveFocus()
        keyClick(Qt.Key_Right, Qt.ShiftModifier)
        fuzzyCompare(surface.trim.trimStart, 1, 0.0001)
        fuzzyCompare(surface.player.seeks[surface.player.seeks.length - 1], 1, 0.0001)

        const last = findNamed(surface, "trimEnd")
        last.forceActiveFocus()
        keyClick(Qt.Key_Left, Qt.ShiftModifier)
        fuzzyCompare(surface.trim.trimEnd, 2, 0.0001)
        fuzzyCompare(surface.player.seeks[surface.player.seeks.length - 1], 2, 0.0001)
    }

    function test_play_keeps_to_the_span() {
        const win = openFilm()
        const surface = findNamed(win.contentItem, "trimSurface")
        surface.trim.setSpan(1, 2)
        surface.player.seek(0)
        mouseClick(button(surface, "Reproducir"))
        fuzzyCompare(surface.player.seeks[surface.player.seeks.length - 1], 1, 0.0001)
        compare(surface.player.state, "reproduciendo")
        // The confirmed position reaches the end of the span: playback stops
        // and goes back to its start.
        surface.player.positionSeconds = 2.01
        compare(surface.player.state, "pausado")
        fuzzyCompare(surface.player.seeks[surface.player.seeks.length - 1], 1, 0.0001)
    }

    function test_a_whole_span_saves_nothing() {
        const win = openFilm()
        const surface = findNamed(win.contentItem, "trimSurface")
        verify(!button(surface, "Guardar ambas").enabled)
        verify(!button(surface, "Guardar solo la editada").enabled)
        surface.trim.setSpan(0.5, 3)
        verify(button(surface, "Guardar ambas").enabled)
        verify(button(surface, "Guardar solo la editada").enabled)
        surface.trim.setSpan(0, 3)
        verify(!button(surface, "Guardar ambas").enabled)
    }

    function test_each_outcome_reaches_save_trim() {
        const win = openFilm()
        const surface = findNamed(win.contentItem, "trimSurface")
        surface.trim.setSpan(1, 2)
        mouseClick(button(surface, "Guardar solo la editada"))
        compare(surface.trim.saves, [true])
        tryVerify(function() { return surface.trim.savedKey.length > 0 })
        // The result is shown as a file to drag out, the film let go first.
        tryVerify(function() { return !surface.visible })
        compare(surface.player.closes, 1)
        const result = findNamed(win.contentItem, "savedResult")
        tryVerify(function() { return result.visible })
        compare(result.Drag.mimeData["text/uri-list"], surface.trim.savedUrl + "\r\n")
        compare(result.Drag.supportedActions, Qt.CopyAction)

        // Opened again, it is a film to trim again; this time a copy.
        const again = button(win.contentItem, "Seguir editando")
        waitForItemPolished(again.parent)
        mouseClick(again)
        tryVerify(function() { return surface.visible })
        tryCompare(surface.trim, "lengthSeconds", 3)
        surface.trim.setSpan(1, 2)
        mouseClick(button(surface, "Guardar ambas"))
        compare(surface.trim.saves, [true, false])
    }

    function test_a_save_in_flight_shows_progress_and_can_be_cancelled() {
        const win = openFilm()
        const surface = findNamed(win.contentItem, "trimSurface")
        surface.trim.setSpan(1, 2)
        surface.trim.holdNextSave = true
        mouseClick(button(surface, "Guardar ambas"))
        verify(surface.trim.saving)
        const progress = findNamed(surface, "trimProgress")
        verify(progress && progress.visible)
        // The row that appeared is laid out before it is clicked.
        waitForItemPolished(progress.parent)
        compare(progress.Accessible.role, Accessible.ProgressBar)
        // Closing waits for the save.
        win.close()
        verify(win.visible)
        mouseClick(button(surface, "Cancelar"))
        compare(surface.trim.cancels, 1)
        verify(!surface.trim.saving)
        verify(!progress.visible)
        verify(surface.visible, "a cancelled trim stays open with its span")
    }

    function test_leaving_with_a_span_chosen_asks_first() {
        const win = openFilm()
        const surface = findNamed(win.contentItem, "trimSurface")
        surface.trim.setSpan(1, 2)
        win.close()
        const asked = question(surface)
        verify(win.visible)
        button(asked, "Guardar ambas")
        button(asked, "Guardar solo la editada")
        mouseClick(button(asked, "Descartar"))
        // The film lets go of its surface before the window goes.
        tryVerify(function() { return !win.visible })
        compare(surface.trim.saves, [])
        verify(surface.player.closes >= 1)
    }

    function test_an_unchanged_film_closes_once_its_surface_let_go() {
        const win = openFilm()
        const surface = findNamed(win.contentItem, "trimSurface")
        win.close()
        tryVerify(function() { return !win.visible })
        verify(surface.player.closes >= 1)
        compare(surface.player.released, 1)
    }

    function test_keep_both_on_the_way_out_saves_and_closes() {
        const win = openFilm()
        const surface = findNamed(win.contentItem, "trimSurface")
        surface.trim.setSpan(1, 2)
        win.close()
        mouseClick(button(question(surface), "Guardar ambas"))
        compare(surface.trim.saves, [false])
        tryVerify(function() { return !win.visible })
    }
}
