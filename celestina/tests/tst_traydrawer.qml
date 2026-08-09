import QtQuick
import QtTest
import CelestinaStyle
import "../qml" as Desktop

// The live observation: `RegisteredStatusNotifierItems` listed four items, all
// four answered `Properties.GetAll`, no unreadable-item warning was logged —
// and the author saw neither Slack nor Solaar in the tray.
//
// Four things could produce that, and only one of them is a defect in this
// file. So each is checked separately rather than assumed:
//
//   1. the model loses an item — covered by `trayitems_test.cpp`;
//   2. the folded drawer shows only what asks for attention, which is what it
//      is for, and none of the four did;
//   3. the open drawer fails to instantiate all four;
//   4. the right flank clips them, which is exactly how the media widget
//      vanished from the left flank once before (`tst_panelflank.qml`).
//
// The shapes below are the ones this session really publishes, captured
// read-only from the bus on 2026-08-07.
TestCase {
    id: testCase

    name: "TrayDrawer"
    visible: true
    // Visibility is asserted below, and an item only reports itself visible
    // once it has a shown window behind it. Without this every `visible` here
    // would be false for a reason that has nothing to do with the tray.
    when: windowShown

    // Slack registers under Chromium's path, publishes pixels and no icon name,
    // and gives no title — so the host shows it under its `Id`.
    readonly property var slack: ({
        "service": ":1.83", "path": "/org/chromium/StatusNotifierItem/1",
        "id": "Slack_status_icon_1", "title": "Slack_status_icon_1",
        "status": "active", "iconName": "", "iconThemePath": "",
        "hasPixmap": true, "hasMenu": true,
        "iconSource": "image://tray/%3A1.83%2Forg%2Fchromium%2FStatusNotifierItem%2F1/1"
    })
    // Solaar names a themed icon and publishes no pixels. On a session whose
    // theme cannot resolve that name the drawer shows its title instead, which
    // is the widest an entry ever gets — so that is the case measured here.
    readonly property var solaar: ({
        "service": ":1.32", "path": "/org/ayatana/NotificationItem/indicator_solaar",
        "id": "indicator-solaar", "title": "Solaar", "status": "active",
        "iconName": "battery-good", "iconThemePath": "",
        "hasPixmap": false, "hasMenu": true, "iconSource": ""
    })
    readonly property var applet: ({
        "service": ":1.22", "path": "/org/ayatana/NotificationItem/nm_applet",
        "id": "nm-applet", "title": "Red", "status": "active",
        "iconName": "nm-signal-100", "iconThemePath": "",
        "hasPixmap": false, "hasMenu": true,
        "iconSource": "image://tray/%3A1.22%2Forg%2Fayatana%2FNotificationItem%2Fnm_applet/1"
    })
    readonly property var blueman: ({
        "service": ":1.26993", "path": "/org/blueman/sni",
        "id": "blueman", "title": "blueman", "status": "active",
        "iconName": "blueman-active", "iconThemePath": "",
        "hasPixmap": false, "hasMenu": true,
        "iconSource": "image://tray/%3A1.26993%2Forg%2Fblueman%2Fsni/1"
    })
    readonly property var session: [slack, solaar, applet, blueman]
    property var lateItems: []

    function entriesOf(drawer) {
        const found = [];
        for (let index = 0; index < drawer.children.length; ++index) {
            if (drawer.children[index].objectName === "celestina-tray-item")
                found.push(drawer.children[index]);
        }
        return found;
    }

    Item {
        id: host

        width: 1920
        height: 40

        Desktop.TrayDrawer {
            id: drawer

            items: testCase.session
        }
    }

    // The panel's real wrapper starts before D-Bus has answered. Its own
    // visibility must follow the independent model, never the child's
    // effective visibility, or the initially hidden parent prevents the child
    // from becoming visible when the late items arrive.
    Item {
        id: lateWrapper

        implicitWidth: lateDrawer.implicitWidth
        implicitHeight: lateDrawer.implicitHeight
        visible: testCase.lateItems.length > 0

        Desktop.TrayDrawer {
            id: lateDrawer

            items: testCase.lateItems
        }
    }

    // The panel, at the width of the author's own output, with every widget the
    // right flank really carries and the readings they really get.
    Item {
        id: panel

        width: 1920
        height: 40

        Desktop.Clock {
            id: clock

            anchors.centerIn: parent
        }

        Desktop.PanelFlank {
            id: rightFlank

            // The same anchors `Panel.qml` gives it, so this measures the
            // shipped layout rather than a copy of it.
            anchors.right: parent.right
            anchors.rightMargin: CelestinaTheme.spaceMd
            anchors.left: clock.right
            anchors.leftMargin: CelestinaTheme.space2xl
            anchors.verticalCenter: parent.verticalCenter
            trailing: true

            Desktop.TrayDrawer {
                id: flankTray

                anchors.verticalCenter: parent.verticalCenter
                items: testCase.session
            }

            Desktop.SessionStatus {
                anchors.verticalCenter: parent.verticalCenter
                network: ({"kind": "wifi", "connection": "Tonys 1"})
                bluetooth: ({"adapter": "on", "count": 1, "first": "S25 Ultra de Antonio"})
                power: ({"active": "performance", "count": 3})
            }

            Desktop.AudioLevel {
                anchors.verticalCenter: parent.verticalCenter
                reading: ({"volume": 42, "muted": false, "micMuted": false,
                           "hasInput": true})
            }

            Desktop.BrightnessLevel {
                anchors.verticalCenter: parent.verticalCenter
                outputName: "DP-1"
                reading: ({"DP-1": 65, "HDMI-A-1": 80})
            }

            Desktop.NotificationIndicator {
                anchors.verticalCenter: parent.verticalCenter
                reading: ({"unread": 3, "quiet": false})
            }

            Desktop.PanelActionButton {
                blurAvailable: true
                iconName: "settings"
                helpText: qsTr("Abrir el centro de control")
            }

            Desktop.PanelActionButton {
                blurAvailable: true
                iconName: "clipboard-paste"
                helpText: qsTr("Abrir el historial del portapapeles")
            }

            Desktop.PanelActionButton {
                blurAvailable: true
                iconName: "power"
                helpText: qsTr("Abrir el menú de sesión")
            }

            Desktop.CaptureButton {
                anchors.verticalCenter: parent.verticalCenter
            }

            Desktop.PhoneStatus {
                blurAvailable: true
                connected: true
                battery: 49
                charging: false
            }
        }
    }

    // 1. The drawer folded is the drawer doing its job: nothing here asks for
    //    attention, so nothing here is shown. The toggle stays, and it is the
    //    only thing that says how many are behind it.
    function test_a_folded_drawer_shows_only_attention_and_keeps_its_count() {
        drawer.open = false;
        compare(testCase.entriesOf(drawer).length, 0);

        const toggle = findChild(drawer, "celestina-tray-toggle");
        verify(toggle, "the drawer has a toggle");
        verify(toggle.visible, "the toggle is visible");
        verify(toggle.width > 0, "the toggle has width: " + toggle.width);
        // The count is in the control's own name, which is what a screen reader
        // reads and what the tooltip would show.
        verify(toggle.helpText.indexOf("4") >= 0, "the toggle names the count: " + toggle.helpText);
        // The row itself is present, so the toggle can be reached at all.
        verify(drawer.visible, "the drawer row is visible");

        // And the bar says how many are behind it. Four registered applications
        // and a bare chevron used to look exactly like none: the count was only
        // in `helpText`, whose visible tooltip this control switches off.
        const count = findChild(drawer, "celestina-tray-count");
        verify(count, "the drawer has a count");
        verify(count.visible, "the count is visible while folded");
        compare(count.text, "4");
    }

    // Opened, the count is noise: the icons it counts are right there.
    function test_an_open_drawer_does_not_repeat_the_count_beside_the_icons() {
        drawer.open = true;
        const count = findChild(drawer, "celestina-tray-count");
        verify(count);
        verify(!count.visible);
        drawer.open = false;
        verify(count.visible);
    }

    // An empty tray is not a tray with a zero on it.
    function test_an_empty_tray_shows_nothing_at_all() {
        drawer.open = false;
        drawer.items = [];
        verify(!drawer.visible);
        const count = findChild(drawer, "celestina-tray-count");
        verify(!count.visible);
        drawer.items = testCase.session;
    }

    function test_late_items_make_the_panel_wrapper_visible() {
        testCase.lateItems = [];
        verify(!lateWrapper.visible);

        testCase.lateItems = testCase.session;
        wait(0);
        verify(lateWrapper.visible);
        verify(lateDrawer.visible);

        testCase.lateItems = [];
    }

    // An item that does ask for attention is shown folded, and only that one.
    function test_a_folded_drawer_still_shows_what_asks_for_attention() {
        drawer.open = false;
        const urgent = JSON.parse(JSON.stringify(testCase.solaar));
        urgent.status = "attention";
        drawer.items = [testCase.slack, urgent, testCase.applet, testCase.blueman];

        const shown = testCase.entriesOf(drawer);
        compare(shown.length, 1);
        compare(shown[0].modelData.title, "Solaar");

        drawer.items = testCase.session;
    }

    // 2. Opened, every registered item is instantiated — including the two the
    //    author did not see.
    function test_an_open_drawer_instantiates_every_registered_item() {
        drawer.open = true;
        const shown = testCase.entriesOf(drawer);
        compare(shown.length, 4);

        const titles = shown.map((entry) => entry.modelData.title);
        verify(titles.indexOf("Slack_status_icon_1") >= 0);
        verify(titles.indexOf("Solaar") >= 0);
        verify(titles.indexOf("Red") >= 0);
        verify(titles.indexOf("blueman") >= 0);

        // Every one of them is something the person can actually reach: an
        // entry with no width is an entry that is not there.
        for (let index = 0; index < shown.length; ++index) {
            verify(shown[index].width > 0);
            verify(shown[index].height > 0);
        }
        drawer.open = false;
    }

    // 3. And the flank has room for them. `PanelFlank` clips, and the tray is
    //    the innermost widget of the trailing flank — so if the row ever
    //    overflows, the tray is the first thing to leave the bar without a
    //    word. That is precisely how the media widget disappeared once before.
    function test_the_open_drawer_is_not_clipped_off_a_1920_output() {
        flankTray.open = true;
        wait(0);

        verify(rightFlank.width > 0);
        compare(testCase.entriesOf(flankTray).length, 4);
        verify(rightFlank.contentWidth <= rightFlank.width);

        // And each entry is inside the flank rather than merely instantiated
        // behind its clip.
        const shown = testCase.entriesOf(flankTray);
        for (let index = 0; index < shown.length; ++index) {
            const at = shown[index].mapToItem(rightFlank, 0, 0);
            verify(at.x >= 0);
            verify(at.x + shown[index].width <= rightFlank.width);
        }
        flankTray.open = false;
    }
}
