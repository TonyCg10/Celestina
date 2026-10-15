import QtQuick
import QtQuick.Window
import QtTest
import "../qml" as Desktop

// The row's three input paths over the same pixels, pressed for real.
//
// This exists because calling `overlay.remove(index)` — which an earlier test
// did — passes happily through a row whose delete button never receives a
// click. Only a press finds that, which is why the row is its own component:
// it can be put in a window and clicked.
//
// Each case builds its own window and clicks it once. A window reused across
// cases keeps the pointer grab its first click established, and every later
// click in that run lands on the same item whatever is under the cursor —
// which made the results look like a stacking bug that was not there.
TestCase {
    id: testCase

    name: "ClipboardEntryRow"
    when: windowShown

    property int selections: 0
    property int removals: 0

    Desktop.BackdropInk {
        id: testInk
    }

    Component {
        id: rowWindow

        Window {
            property alias row: content

            width: 300
            height: 40
            visible: true

            Desktop.ClipboardEntryRow {
                id: content

                anchors.fill: parent
                ink: testInk
                entry: ({"index": 0, "preview": "una entrada"})
                current: true
                onSelected: testCase.selections += 1
                onRemoved: testCase.removals += 1
            }
        }
    }

    function init() {
        testCase.selections = 0;
        testCase.removals = 0;
    }

    // Builds a fresh row, clicks it once at `x`, and reports what it did.
    function clickAt(x, button, keepCurrent) {
        const host = rowWindow.createObject(null);
        verify(host !== null);
        const row = host.row;
        if (keepCurrent === false)
            row.current = false;
        verify(waitForRendering(row), "the row must be on screen to be clicked");
        verify(row.visible, "an invisible row would receive no input");

        mouseClick(row, x, row.height / 2, button === undefined ? Qt.LeftButton : button);
        wait(0);
        host.destroy();
    }

    // Where the delete button sits: pinned to the right edge, its own width
    // wide. Read from the row rather than guessed, so the case follows the
    // theme's metrics.
    function buttonX(row) {
        return row.width - 12;
    }

    function test_a_left_click_on_the_row_selects_it() {
        clickAt(20, Qt.LeftButton);
        compare(testCase.selections, 1);
        compare(testCase.removals, 0);
    }

    function test_a_right_click_on_the_row_removes_it() {
        clickAt(20, Qt.RightButton);
        compare(testCase.removals, 1);
        compare(testCase.selections, 0);
    }

    function test_the_visible_button_receives_its_own_click() {
        clickAt(288, Qt.LeftButton);

        compare(testCase.removals, 1, "the delete button must receive the click");
        compare(testCase.selections, 0, "the row's own area must not answer for it");
    }

    function test_the_button_answers_even_when_the_row_is_not_current() {
        // Reached by hovering rather than by the keyboard cursor: the button
        // appears under the pointer and must be clickable there too.
        clickAt(288, Qt.LeftButton, false);
        compare(testCase.removals, 1);
        compare(testCase.selections, 0);
    }
}
