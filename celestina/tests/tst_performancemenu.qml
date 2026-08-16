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

    Component {
        id: attachedMenuComponent

        Desktop.PerformanceMenu {
            visible: true
            outputName: "scaled-test-output"
            providerSource: fakeSource
            reducedMotion: false
            shellScale: 1.15
            anchoredFromPanel: true
            openerRect: Qt.rect(520, 5, 60, 30)
            attachmentAnchorRect: Qt.rect(541, 11, 18, 18)
            attachmentStartY: 40
        }
    }

    SignalSpy {
        id: menuClosed

        target: performanceMenu.menu
        signalName: "closed"
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
        if (!performanceMenu.menu.visible) {
            performanceMenu.menu.open();
        }
        tryCompare(performanceMenu.menu, "opened", true);
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

    function test_floating_rows_wait_for_the_shared_presentation_gate() {
        const field = findChild(
                    performanceMenu, "celestina-soft-menu-field");
        verify(field);
        verify(performanceMenu.menu.visible);

        // Popup.Item reparents the row viewport outside SoftMenuField. Reset a
        // live floating instance to prove those real rows obey the same gate,
        // rather than exposing Qt's stock popup while field/glass remain dark.
        field.resetForReuse();
        compare(field.revealed, false);
        compare(performanceMenu.menu.opacity, 0);

        field.revealNow();
        compare(field.revealed, true);
        compare(performanceMenu.menu.opacity, 1);
    }

    function test_attached_rows_move_inside_a_fixed_scaled_viewport() {
        const attached = attachedMenuComponent.createObject(testCase);
        verify(attached);
        // Component completion initially asks for the card-sized carrier. A
        // real layer configure replaces that with the output extent before it
        // presents; model that configure before the queued popup open runs.
        attached.width = 800;
        attached.height = 600;
        attached.menuX = 420;
        attached.menuY = 40;
        tryCompare(attached.menu, "opened", true);

        const field = findChild(attached, "celestina-soft-menu-field");
        verify(field);
        verify(attached.rowsViewport);
        verify(attached.rowsContent);
        verify(attached.rowsViewport.clip);

        // Freeze a real entry geometry without touching the close lifecycle.
        // Put the moving rows halfway through the panel strip: the old code
        // moved the viewport and its clip there too, so this is the frame that
        // visibly painted over the bar.
        const fall = findChild(field, "celestina-attachment-drop-fall");
        verify(fall);
        field.revealNow();
        field.fallQueued = false;
        field.hasFallen = true;
        fall.stop();
        const targetRideY = attached.attachmentStartY / 2;
        const progress = 1
                + (targetRideY - attached.cardY) / field.entryTravel;
        verify(progress > 0 && progress < 1);
        field.attachmentProgress = progress;
        wait(0);
        verify(attached.rowsCut > 0);

        const viewportDuring = attached.rowsViewport.mapToItem(
                    attached.contentItem, 0, 0).y;
        const rowsDuring = attached.rowsContent.mapToItem(
                    attached.rowsViewport, 0, 0).y;
        // The Popup viewport itself starts at or below the panel seam in the
        // window's scaled coordinates. Only its internal rows move upward.
        verify(viewportDuring
               >= attached.attachmentStartY * attached.shellScale - 0.5);

        const shot = grabImage(attached.contentItem);
        const seamPixels = Math.floor(
                    attached.attachmentStartY * attached.shellScale);
        for (let y = 0; y < seamPixels - 1; ++y) {
            for (let x = 0; x < attached.width; ++x) {
                verify((shot.pixel(x, y) & 0xFF000000) === 0,
                       "popup painted above the seam at " + x + "," + y);
            }
        }

        const cut = attached.rowsCut;
        field.attachmentProgress = 1;
        wait(0);
        const viewportSettled = attached.rowsViewport.mapToItem(
                    attached.contentItem, 0, 0).y;
        const rowsSettled = attached.rowsContent.mapToItem(
                    attached.rowsViewport, 0, 0).y;

        verify(viewportSettled >= viewportDuring);
        fuzzyCompare(rowsSettled - rowsDuring, cut, 0.01);
        attached.destroy();
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
        menuClosed.clear();
        reading.triggered();

        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].provider, "sysmon");
        compare(fakeSource.sent[0].verb, "open-monitor");
        tryCompare(menuClosed, "count", 1);
        compare(performanceMenu.menu.visible, false);
    }

    function test_an_unavailable_reading_opens_nothing() {
        fakeSource.publish({});

        const unavailable = rowOfKind("unavailable");
        verify(unavailable);
        // There is nothing being measured, so there is nothing to click into.
        verify(!unavailable.actionable);
        verify(performanceMenu.menu.visible);
        mouseClick(unavailable, unavailable.width / 2,
                   unavailable.visualHeight / 2, Qt.LeftButton);
        compare(fakeSource.sent.length, 0);
        verify(performanceMenu.menu.visible);
    }
}
