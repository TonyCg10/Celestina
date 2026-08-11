import QtQuick
import QtTest
import "../qml" as Desktop

// The screenshot surface keeps the real Menu lifecycle: ordered semantic rows,
// one actionable capture request and normal dismissal after activation.
TestCase {
    id: testCase

    name: "CaptureMenu"

    Desktop.CaptureMenu {
        id: menu

        outputName: "test-output"
        reducedMotion: true
    }

    SignalSpy {
        id: captureSpy

        target: menu
        signalName: "captureRequested"
    }

    SignalSpy {
        id: dismissedSpy

        target: menu
        signalName: "dismissed"
    }

    function init() {
        captureSpy.clear();
        dismissedSpy.clear();
        if (!menu.menu.visible)
            menu.menu.open();
    }

    function rowAt(index) {
        return menu.menu.itemAt(index);
    }

    function test_it_keeps_an_extensible_ordered_hierarchy() {
        compare(menu.entries.length, 3);
        compare(menu.entries[0].kind, "header");
        compare(menu.entries[1].kind, "section");
        compare(menu.entries[2].kind, "capture");
        compare(menu.menu.count, 3);

        const header = testCase.rowAt(0);
        const section = testCase.rowAt(1);
        const action = testCase.rowAt(2);
        verify(header.header);
        verify(!header.actionable);
        compare(header.text, qsTr("Caja de herramientas"));
        compare(header.iconName, "toolbox");
        verify(section.sectionLabel);
        verify(!section.actionable);
        verify(action.actionable);
        compare(action.text, qsTr("Capturar pantalla"));
        compare(action.iconName, "scissors");
    }

    function test_the_action_requests_once_and_closes_the_real_menu() {
        const action = testCase.rowAt(2);
        verify(action !== null);

        action.triggered();

        compare(captureSpy.count, 1);
        tryCompare(menu.menu, "visible", false);
        tryCompare(dismissedSpy, "count", 1);
    }
}
