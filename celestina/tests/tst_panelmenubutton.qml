import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

// Every direct panel menu opener delegates its pointer, keyboard and geometry
// contract here. Consumers may replace the content, but a press must never be
// visually silent and every request must carry the real opener rectangle.
TestCase {
    id: testCase

    name: "PanelMenuButton"
    when: windowShown
    visible: true
    width: 120
    height: 80

    Desktop.BackdropInk {
        id: testInk
    }

    Desktop.PanelMenuButton {
        id: button

        anchors.centerIn: parent
        width: 32
        height: 28
        ink: testInk
        text: "menu"
    }

    SignalSpy {
        id: requests

        target: button
        signalName: "menuRequested"
    }

    function init() {
        requests.clear();
    }

    function test_a_pointer_press_has_visible_feedback() {
        mousePress(button, button.width / 2, button.height / 2,
                   Qt.LeftButton);
        verify(button.down);
        tryCompare(button.background, "color", CelestinaTheme.surfaceStrong);

        mouseRelease(button, button.width / 2, button.height / 2,
                     Qt.LeftButton);
        verify(!button.down);
        compare(requests.count, 1);
    }

    function test_every_request_carries_the_real_opener_rectangle() {
        button.click();
        compare(requests.count, 1);
        const arguments = requests.signalArguments[0];
        compare(arguments[2], button.width);
        compare(arguments[3], button.height);

        const expected = button.mapToGlobal(0, 0);
        compare(arguments[0], expected.x);
        compare(arguments[1], expected.y);
    }

    function test_return_uses_the_same_request_path() {
        testCase.forceActiveFocus(Qt.MouseFocusReason);
        button.forceActiveFocus(Qt.TabFocusReason);
        verify(button.visualFocus);
        keyClick(Qt.Key_Return);
        compare(requests.count, 1);
    }
}
