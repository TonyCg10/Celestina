import QtQuick
import QtTest
import CelestinaStyle

// The gutter's two ways of knowing where the lines are. Without a lineSource
// it scans the surface's text itself; with one, the host's document core
// answers and the gutter must neither re-scan nor race the widget — its
// relayout beat is the source's lineRevision, which arrives after the core
// has absorbed the edit.
TestCase {
    id: testCase

    name: "CelestinaLineGutter"
    width: 240
    height: 160
    visible: true
    when: windowShown

    // Not `surface`: naming the id after the property it fills makes an
    // `x: x` auto-binding, which the architecture scanner refuses at depth.
    TextEdit {
        id: sheet

        width: 160
        height: 120
        font.pixelSize: CelestinaTheme.fontBody
        text: "uno\ndos\ntres"
    }

    // A host core in miniature: three lines whose offsets it already knows,
    // and the revision beat it owes the gutter after each change.
    QtObject {
        id: stubSource

        property int lineCount: 3
        property int lineRevision: 0
        property var starts: [0, 4, 8]
        // Mutated through the object rather than declared as its own notifiable
        // property: the gutter calls this function while its bindings are being
        // evaluated, and a change signal emitted mid-evaluation reads as a
        // binding loop that the component does not actually have.
        property var tally: ({ asks: 0 })

        function lineStartUtf16(line) {
            stubSource.tally.asks += 1
            return stubSource.starts[Math.max(0, Math.min(line, stubSource.starts.length - 1))]
        }
    }

    CelestinaLineGutter {
        id: scanning

        surface: sheet
        viewportY: 0
        viewportHeight: surface.height
    }

    CelestinaLineGutter {
        id: sourced

        surface: sheet
        lineSource: stubSource
        viewportY: 0
        viewportHeight: surface.height
    }

    function test_a_self_scanning_gutter_still_indexes_the_text() {
        compare(scanning.lineCount, 3)
        compare(scanning.offsetOf(0), 0)
        compare(scanning.offsetOf(1), 4)
        compare(scanning.offsetOf(2), 8)
        // Clamped at both ends rather than read out of range.
        compare(scanning.offsetOf(-1), 0)
        compare(scanning.offsetOf(9), 8)
    }

    function test_b_a_sourced_gutter_asks_the_source_not_the_text() {
        compare(sourced.lineCount, 3)
        stubSource.tally.asks = 0
        compare(sourced.offsetOf(1), 4)
        compare(sourced.offsetOf(9), 8)
        verify(stubSource.tally.asks >= 2, "the offsets did not come from the source")
        // The self-scan never ran: its index still holds only the default.
        compare(sourced.lineStarts.length, 1)
    }

    function test_c_the_source_revision_is_the_relayout_beat() {
        const before = sourced.layoutRevision
        stubSource.starts = [0, 4, 8, 13]
        stubSource.lineCount = 4
        stubSource.lineRevision += 1
        verify(sourced.layoutRevision > before, "the revision beat did not relayout")
        compare(sourced.lineCount, 4)
        compare(sourced.offsetOf(3), 13)
    }

    function test_d_editing_the_text_does_not_make_a_sourced_gutter_rescan() {
        sheet.text = "uno\ndos\ntres\ncuatro\ncinco"
        // The widget's own textChanged fired; a sourced gutter leaves the
        // scan alone and keeps answering from the source.
        compare(sourced.lineStarts.length, 1)
        // The scanning gutter, by contrast, followed the text.
        compare(scanning.lineCount, 5)
    }
}
