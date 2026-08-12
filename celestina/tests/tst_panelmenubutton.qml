import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

// Every direct panel menu opener delegates its pointer, keyboard and geometry
// contract here. Consumers may replace the content, but a press must never be
// visually silent. Placement follows the complete pointer target while the
// icon-sized anchor positions the background attachment's narrow waist.
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
        attachmentAnchor: testAnchor

        contentItem: Item {
            Item {
                id: testAnchor

                anchors.centerIn: parent
                width: 18
                height: 18
            }
        }
    }

    SignalSpy {
        id: requests

        target: button
        signalName: "menuRequested"
    }

    function init() {
        requests.clear();
        button.menuOpen = false;
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

    function test_every_request_separates_the_control_from_its_icon_anchor() {
        button.click();
        compare(requests.count, 1);
        const arguments = requests.signalArguments[0];
        const openerAt = button.mapToGlobal(0, 0);
        compare(arguments[0].x, openerAt.x);
        compare(arguments[0].y, openerAt.y);
        compare(arguments[0].width, button.width);
        compare(arguments[0].height, button.height);

        const anchor = button.attachmentAnchorGlobalRectNow();
        compare(arguments[1], anchor);
        compare(anchor.width, 18);
        compare(anchor.height, 18);
        compare(anchor.x,
                arguments[0].x + (arguments[0].width - anchor.width) / 2);
        compare(anchor.y,
                arguments[0].y + (arguments[0].height - anchor.height) / 2);
        verify(button.isPanelAttachmentSource);
        verify(anchor.width < arguments[0].width);
    }

    function test_an_open_menu_holds_the_exact_hover_feedback() {
        mouseMove(button, button.width / 2, button.height / 2);
        tryCompare(button, "hovered", true);
        tryCompare(button.background, "color", testInk.controlFill);
        const hoverColor = button.background.color;
        const hoverRadius = button.background.radius;
        const restingWidth = button.width;
        const restingHeight = button.height;

        mouseMove(testCase, 0, 0);
        tryCompare(button, "hovered", false);
        tryCompare(button.background, "color", CelestinaTheme.clear);

        button.menuOpen = true;
        tryCompare(button.background, "color", hoverColor);
        compare(button.background.radius, hoverRadius);
        compare(button.width, restingWidth);
        compare(button.height, restingHeight);

        button.menuOpen = false;
        tryCompare(button.background, "color", CelestinaTheme.clear);
    }

    function test_return_uses_the_same_request_path() {
        testCase.forceActiveFocus(Qt.MouseFocusReason);
        button.forceActiveFocus(Qt.TabFocusReason);
        verify(button.visualFocus);
        keyClick(Qt.Key_Return);
        compare(requests.count, 1);
    }
}
