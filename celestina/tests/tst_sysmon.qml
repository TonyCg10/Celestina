import QtQuick
import QtTest
import "../qml" as Desktop

// The performance reading occupies one panel icon; its percentages move into
// the contextual menu without disappearing from assistive technology.
TestCase {
    id: testCase

    name: "SysMon"

    property var currentReading: ({"cpu": 6, "ram": 24})

    Desktop.BackdropInk {
        id: testInk
    }

    Desktop.SysMon {
        id: performance

        ink: testInk
        blurAvailable: true
        reading: testCase.currentReading
    }

    SignalSpy {
        id: menus

        target: performance
        signalName: "menuRequested"
    }

    function init() {
        testCase.currentReading = {"cpu": 6, "ram": 24};
        menus.clear();
    }

    function test_it_is_one_fixed_icon_with_both_values_in_its_name() {
        verify(performance.hasReading);
        compare(performance.iconName, "cpu");
        compare(performance.implicitWidth, performance.implicitHeight);
        verify(performance.Accessible.name.indexOf("6") >= 0);
        verify(performance.Accessible.name.indexOf("24") >= 0);
        verify(performance.Accessible.description.indexOf(qsTr("menú")) >= 0);
    }

    function test_it_reports_the_real_opener_geometry() {
        performance.click();

        compare(menus.count, 1);
        compare(menus.signalArguments[0][2], performance.width);
        compare(menus.signalArguments[0][3], performance.height);
    }

    function test_an_incomplete_reading_is_not_presented_as_current() {
        testCase.currentReading = {"cpu": 6};

        verify(!performance.hasReading);
        verify(!performance.visible);

        testCase.currentReading = undefined;
        verify(!performance.hasReading);
        verify(!performance.visible);
    }
}
