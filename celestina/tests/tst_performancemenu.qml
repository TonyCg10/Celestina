import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

// The contextual performance menu follows the live provider frame and keeps
// the existing external-monitor request behind the aggregate provider bridge.
TestCase {
    id: testCase

    name: "PerformanceMenu"

    QtObject {
        id: fakeSource

        property bool available: true
        property var providers: ({})
        property int revision: 0
        property var sent: []

        signal changed()

        function publish(next) {
            fakeSource.providers = next;
            fakeSource.revision = fakeSource.revision + 1;
            fakeSource.changed();
        }

        function sendCommand(provider, verb, options) {
            fakeSource.sent.push({
                "provider": provider, "verb": verb, "options": options
            });
            return fakeSource.sent.length;
        }
    }

    Desktop.PerformanceMenu {
        id: performanceMenu

        visible: true
        outputName: "test-output"
        providerSource: fakeSource
        reducedMotion: true
    }

    function entriesOfKind(kind) {
        const found = [];
        for (let index = 0; index < performanceMenu.entries.length; ++index) {
            if (performanceMenu.entries[index].kind === kind)
                found.push(performanceMenu.entries[index]);
        }
        return found;
    }

    function rowOfKind(kind) {
        for (let index = 0; index < performanceMenu.entries.length; ++index) {
            if (performanceMenu.entries[index].kind === kind)
                return performanceMenu.menu.itemAt(index);
        }
        return null;
    }

    function init() {
        fakeSource.sent = [];
        fakeSource.publish({"sysmon": {"cpu": 6, "ram": 24}});
        if (!performanceMenu.menu.visible)
            performanceMenu.menu.open();
    }

    function test_it_uses_the_shared_contextual_menu_rhythm() {
        compare(performanceMenu.itemSpacing, CelestinaTheme.spaceSm);
        compare(performanceMenu.headerBodyGap, CelestinaTheme.spaceMd);
        compare(performanceMenu.rowVerticalInset, CelestinaTheme.spaceXs);
        compare(performanceMenu.menu.modal, false);
        verify(findChild(performanceMenu, "celestina-menu-header"));
        verify(findChild(performanceMenu,
                         "celestina-compositor-glass-region"));
    }

    function test_cpu_and_memory_follow_the_live_provider_revision() {
        verify(performanceMenu.hasReading);
        compare(entriesOfKind("metric").length, 2);
        compare(rowOfKind("metric").text, qsTr("Procesador"));
        compare(rowOfKind("metric").note, qsTr("6 %"));
        compare(performanceMenu.menu.itemAt(3).text, qsTr("Memoria"));
        compare(performanceMenu.menu.itemAt(3).note, qsTr("24 %"));

        fakeSource.publish({"sysmon": {"cpu": 71, "ram": 52}});

        tryCompare(rowOfKind("metric"), "note", qsTr("71 %"));
        tryCompare(performanceMenu.menu.itemAt(3), "note", qsTr("52 %"));
    }

    function test_a_withdrawn_or_incomplete_reading_is_explicit() {
        fakeSource.publish({});

        verify(!performanceMenu.hasReading);
        compare(performanceMenu.readingLine, qsTr("Sin lectura actual"));
        compare(entriesOfKind("metric").length, 0);
        compare(entriesOfKind("unavailable").length, 1);
        compare(rowOfKind("unavailable").text,
                qsTr("Sin lectura de rendimiento"));

        fakeSource.publish({"sysmon": {"cpu": 9}});
        verify(!performanceMenu.hasReading);
        compare(entriesOfKind("unavailable").length, 1);
    }

    function test_a_reading_is_its_own_way_into_the_monitor() {
        // The separate tools section is gone: the thing being measured opens
        // the monitor that measures it, so there is no trailing section and
        // no row whose only purpose is to be a link.
        compare(entriesOfKind("section").length, 1);
        compare(entriesOfKind("monitor").length, 0);

        const reading = rowOfKind("metric");
        verify(reading);
        verify(reading.actionable);
        verify(performanceMenu.menu.visible);

        // Physical pointer and keyboard delivery for every contextual menu is
        // covered by IndicatorMenuTest. Invoke the row's activation contract
        // here so this case isolates the exact provider command and teardown.
        reading.triggered();

        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].provider, "sysmon");
        compare(fakeSource.sent[0].verb, "open-monitor");
        tryCompare(performanceMenu.menu, "visible", false);
    }

    function test_an_unavailable_reading_opens_nothing() {
        fakeSource.publish({});

        const unavailable = rowOfKind("unavailable");
        verify(unavailable);
        // There is nothing being measured, so there is nothing to click into.
        verify(!unavailable.actionable);
        unavailable.triggered();
        compare(fakeSource.sent.length, 0);
        verify(performanceMenu.menu.visible);
    }
}
