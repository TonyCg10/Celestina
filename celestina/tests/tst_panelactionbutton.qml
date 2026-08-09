import QtQuick
import QtTest
import "../qml" as Desktop

// A permanent panel entry point must remain a real button and must contribute
// its glass region without owning the overlay it opens.
TestCase {
    id: testCase

    name: "PanelActionButton"

    Desktop.PanelActionButton {
        id: button

        blurAvailable: true
        iconName: "settings"
        helpText: qsTr("Abrir el centro de control")
    }

    SignalSpy {
        id: clicks

        target: button
        signalName: "clicked"
    }

    function test_it_keeps_one_icon_and_reports_clicks() {
        compare(button.iconName, "settings");
        verify(button.implicitWidth > 0);
        button.click();
        compare(clicks.count, 1);
    }
}
