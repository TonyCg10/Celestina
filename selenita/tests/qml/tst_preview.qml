import QtQuick
import QtTest 1.3
import org.celestina.selenita 1.0

// The corner preview over the stand-in controller: it appears on
// `previewShown`, closes five seconds later unless the pointer rests on it,
// its × dismisses and a click hands the file to Fluorita, a new result
// replaces the content, the drag offers the file's URI as a copy, a trashed
// file closes it, reduced motion makes it appear at once, and the window is
// the frameless, unfocused one niri's rule matches by its title.
TestCase {
    id: testCase
    name: "Preview"
    when: windowShown

    readonly property string shotKey: "/fake/Captura 1.png"
    readonly property string shotUri: "file:///fake/Captura%201.png"
    readonly property string clipKey: "/fake/Clip 2.mp4"
    readonly property string clipUri: "file:///fake/Clip%202.mp4"

    Component {
        id: previewComponent
        PreviewWindow {}
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

    function init() {
        SelenitaController.reset()
        CelestinaTheme.reducedMotion = false
    }

    function cleanup() {
        CelestinaTheme.reducedMotion = false
    }

    function makePreview() {
        const preview = createTemporaryObject(previewComponent, testCase)
        verify(preview)
        return preview
    }

    // The window is on screen and the pointer is away from it: a pointer
    // event sent before the first frame is lost, and the pointer a previous
    // test left over the same spot would hold the preview.
    function settle(preview) {
        verify(waitForRendering(preview.contentItem))
        const card = findNamed(preview.contentItem, "previewCard")
        mouseMove(card, card.width + 50, card.height + 50)
        tryVerify(function() { return !preview.hovered })
        return card
    }

    function showShot() {
        SelenitaController.showPreview(testCase.shotKey, "screenshot",
                                       Qt.resolvedUrl("stubs/pixel.png"), "",
                                       testCase.shotUri, 1)
    }

    function showClip(poster) {
        SelenitaController.showPreview(testCase.clipKey, "recording", poster, "0:07",
                                       testCase.clipUri, 0.5625)
    }

    function test_the_window_is_the_one_niri_matches() {
        const preview = makePreview()
        compare(preview.title, qsTr("Vista previa"))
        verify(preview.flags & Qt.FramelessWindowHint, "frameless")
        verify(preview.flags & Qt.WindowDoesNotAcceptFocus, "never takes the focus")
        compare(preview.transientParent, null)
        verify(!preview.visible, "hidden until a result")
    }

    function test_it_appears_on_preview_shown() {
        const preview = makePreview()
        testCase.showShot()
        verify(preview.visible)
        const picture = findNamed(preview.contentItem, "previewPicture")
        compare(picture.source, Qt.resolvedUrl("stubs/pixel.png"))
        verify(preview.holding, "the five seconds run")
        compare(preview.holdTime, 5000)
    }

    function test_it_closes_five_seconds_later() {
        const preview = makePreview()
        testCase.showShot()
        testCase.settle(preview)
        wait(4500)
        verify(preview.visible, "still there before five seconds")
        tryVerify(function() { return !preview.visible }, 1500)
        compare(SelenitaController.calls, ["dismissPreview:" + testCase.shotKey])
        verify(!SelenitaController.previewVisible)
    }

    function test_the_pointer_holds_it_and_leaving_lets_it_go() {
        const preview = makePreview()
        testCase.showShot()
        const card = testCase.settle(preview)
        mouseMove(card, card.width / 2, card.height / 2)
        tryVerify(function() { return preview.hovered })
        verify(!preview.holding, "the timer stops under the pointer")
        mouseMove(card, card.width + 50, card.height + 50)
        tryVerify(function() { return !preview.hovered })
        verify(preview.holding, "the timer restarts on leave")
        verify(preview.visible)
    }

    function test_the_close_button_dismisses() {
        const preview = makePreview()
        testCase.showShot()
        testCase.settle(preview)
        const close = findNamed(preview.contentItem, "previewClose")
        verify(close.visible)
        mouseClick(close)
        compare(SelenitaController.calls, ["dismissPreview:" + testCase.shotKey])
        tryVerify(function() { return !preview.visible })
    }

    function test_a_click_hands_the_file_to_fluorita_and_closes() {
        const preview = makePreview()
        testCase.showShot()
        const card = testCase.settle(preview)
        mouseClick(card, card.width / 3, card.height * 2 / 3)
        compare(SelenitaController.calls, ["editPreview:" + testCase.shotKey])
        tryVerify(function() { return !preview.visible })
    }

    function test_a_second_result_replaces_the_first() {
        const preview = makePreview()
        testCase.showShot()
        const glyph = findNamed(preview.contentItem, "previewFilm")
        const duration = findNamed(preview.contentItem, "previewDuration")
        verify(!glyph.visible)
        testCase.showClip(Qt.resolvedUrl("stubs/pixel.png"))
        verify(preview.visible)
        verify(preview.holding)
        compare(SelenitaController.previewKey, testCase.clipKey)
        verify(glyph.visible, "a recording wears the film glyph")
        verify(duration.visible)
        compare(duration.text, "0:07")
        compare(findNamed(preview.contentItem, "previewPicture").source,
                Qt.resolvedUrl("stubs/pixel.png"))
        compare(preview.height, 140, "a wide picture is clamped at the lowest height")
    }

    function test_a_recording_without_a_poster_shows_the_film_glyph_alone() {
        const preview = makePreview()
        testCase.showClip("")
        const picture = findNamed(preview.contentItem, "previewPicture")
        verify(!picture.visible)
        verify(findNamed(preview.contentItem, "previewPlaceholder").visible)
        compare(findNamed(preview.contentItem, "previewDuration").text, "0:07")
    }

    function test_the_drag_offers_the_file_as_a_copy() {
        const preview = makePreview()
        testCase.showClip(Qt.resolvedUrl("stubs/pixel.png"))
        const card = findNamed(preview.contentItem, "previewCard")
        compare(card.Drag.mimeData["text/uri-list"], testCase.clipUri)
        compare(card.Drag.supportedActions, Qt.CopyAction)
        compare(card.Drag.proposedAction, Qt.CopyAction)
        compare(card.Drag.dragType, Drag.Automatic)
    }

    function test_a_trashed_file_closes_it() {
        const preview = makePreview()
        testCase.showShot()
        SelenitaController.deleteEntry(testCase.shotKey)
        tryVerify(function() { return !preview.visible })
        verify(SelenitaController.calls.indexOf("dismissPreview:" + testCase.shotKey) < 0)
    }

    function test_reduced_motion_shows_and_hides_at_once() {
        CelestinaTheme.reducedMotion = true
        const preview = makePreview()
        testCase.showShot()
        const card = findNamed(preview.contentItem, "previewCard")
        compare(card.opacity, 1)
        compare(card.offset, 0)
        SelenitaController.dismissPreview()
        verify(!preview.visible, "gone at once")
    }

    function test_with_motion_it_fades_in() {
        const preview = makePreview()
        testCase.showShot()
        const card = findNamed(preview.contentItem, "previewCard")
        verify(card.opacity < 1, "the entry fades")
        tryCompare(card, "opacity", 1)
        tryCompare(card, "offset", 0)
    }

    function test_a_failed_hand_off_is_said_in_the_preview() {
        const preview = makePreview()
        testCase.showShot()
        const notice = findNamed(preview.contentItem, "previewNotice")
        verify(!notice.visible)
        const said = qsTr("No se ha podido abrir Fluorita.")
        SelenitaController.previewNotice = said
        verify(notice.visible)
        compare(notice.text, said)
    }
}
