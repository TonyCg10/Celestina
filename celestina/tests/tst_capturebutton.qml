import CelestinaStyle
import QtQuick
import QtQuick.Controls
import QtTest
import "../qml" as Desktop

// The panel capture control opens a menu and keeps its bounded refusal
// feedback; it never bypasses that menu by requesting a capture directly.
TestCase {
    id: testCase

    name: "CaptureButton"
    when: windowShown
    visible: true
    width: 120
    height: 80

    Desktop.BackdropInk {
        id: testInk
    }

    Desktop.CaptureButton {
        id: button

        anchors.centerIn: parent
        ink: testInk
        blurAvailable: true
    }

    SignalSpy {
        id: menuSpy

        target: button
        signalName: "menuRequested"
    }

    function init() {
        menuSpy.clear();
        button.failed = false;
    }

    function test_it_opens_the_menu_with_its_real_geometry() {
        compare(button.iconName, "toolbox");

        button.click();

        compare(menuSpy.count, 1);
        const arguments = menuSpy.signalArguments[0];
        const expected = button.mapToGlobal(0, 0);
        compare(arguments[0].x, expected.x);
        compare(arguments[0].y, expected.y);
        compare(arguments[0].width, button.width);
        compare(arguments[0].height, button.height);
        const anchor = arguments[1];
        compare(anchor, button.attachmentAnchorGlobalRectNow());
        compare(anchor.width, 18);
        compare(anchor.height, 18);
        compare(anchor.x,
                arguments[0].x + (arguments[0].width - anchor.width) / 2);
        compare(anchor.y,
                arguments[0].y + (arguments[0].height - anchor.height) / 2);
        verify(button.isPanelAttachmentSource);
    }

    function test_a_refusal_keeps_the_existing_bounded_feedback() {
        compare(button.role, CelestinaButton.Ghost);
        compare(button.helpText, qsTr("Caja de herramientas"));

        button.reportFailure();

        verify(button.failed);
        compare(button.role, CelestinaButton.Destructive);
        compare(button.helpText, qsTr("No se pudo pedir la captura"));
        compare(menuSpy.count, 0);
    }

    function test_shell_help_text_never_becomes_a_hover_tooltip() {
        verify(button.helpText.length > 0);
        compare(button.Accessible.name, button.helpText);
        mouseMove(button, button.width / 2, button.height / 2);
        tryCompare(button, "hovered", true);
        compare(button.ToolTip.text, "");
        compare(button.ToolTip.visible, false);
    }
}
