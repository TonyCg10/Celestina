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
//   2. the compact opener shows only what asks for attention, while the full
//      inventory belongs to a separate contextual menu;
//   3. the right flank clips the compact opener, which is exactly how the media
//      widget vanished from the left flank once before (`tst_panelflank.qml`).
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
        "preferenceKey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
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
        "preferenceKey": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "iconName": "battery-good", "iconThemePath": "",
        "hasPixmap": false, "hasMenu": true, "iconSource": ""
    })
    readonly property var applet: ({
        "service": ":1.22", "path": "/org/ayatana/NotificationItem/nm_applet",
        "id": "nm-applet", "title": "Red", "status": "active",
        "preferenceKey": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "iconName": "nm-signal-100", "iconThemePath": "",
        "hasPixmap": false, "hasMenu": true,
        "iconSource": "image://tray/%3A1.22%2Forg%2Fayatana%2FNotificationItem%2Fnm_applet/1"
    })
    readonly property var blueman: ({
        "service": ":1.26993", "path": "/org/blueman/sni",
        "id": "blueman", "title": "blueman", "status": "active",
        "preferenceKey": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        "iconName": "blueman-active", "iconThemePath": "",
        "hasPixmap": false, "hasMenu": true,
        "iconSource": "image://tray/%3A1.26993%2Forg%2Fblueman%2Fsni/1"
    })
    readonly property var session: [slack, solaar, applet, blueman]
    property var lateItems: []

    Desktop.BackdropInk {
        id: testInk
    }

    function entriesOf(drawer) {
        const found = [];
        function visit(node) {
            for (let index = 0; index < node.children.length; ++index) {
                const child = node.children[index];
                if (child.objectName === "celestina-tray-item")
                    found.push(child);
                visit(child);
            }
        }
        visit(drawer);
        return found;
    }

    function glassRegions(item) {
        const found = [];

        function visit(node) {
            if (node.objectName === "celestina-compositor-glass-region")
                found.push(node);

            for (let index = 0; index < node.children.length; ++index)
                visit(node.children[index]);
        }

        visit(item);
        return found;
    }

    function visibleGlassRegions(item) {
        const regions = testCase.glassRegions(item);
        const visible = [];
        for (let index = 0; index < regions.length; ++index) {
            if (regions[index].visible)
                visible.push(regions[index]);
        }
        return visible;
    }

    Item {
        id: host

        width: 1920
        height: 40

        Desktop.TrayDrawer {
            id: drawer

            ink: testInk
            items: testCase.session
            preferences: []
        }
    }

    SignalSpy {
        id: drawerRequests

        target: drawer
        signalName: "drawerRequested"
    }

    SignalSpy {
        id: trayActivations

        target: drawer
        signalName: "activated"
    }

    SignalSpy {
        id: trayItemMenus

        target: drawer
        signalName: "menuRequested"
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

            ink: testInk
            items: testCase.lateItems
            preferences: []
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

            ink: testInk
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
            height: panel.height
            trailing: true

            Desktop.TrayDrawer {
                id: flankTray

                ink: testInk
                anchors.verticalCenter: parent.verticalCenter
                items: testCase.session
                preferences: []
            }

            Desktop.PanelCluster {
                id: connectivityCluster

                blurAvailable: true
                ink: testInk
                spacing: CelestinaTheme.spaceMd

                Desktop.SessionStatus {
                    ink: testInk
                    network: ({"kind": "wifi", "connection": "Tonys 1"})
                    bluetooth: ({"adapter": "on", "count": 1,
                                 "first": "S25 Ultra de Antonio"})
                }
            }

            Desktop.PanelCluster {
                id: levelCluster

                blurAvailable: true
                ink: testInk
                spacing: CelestinaTheme.spaceMd

                Desktop.AudioLevel {
                    ink: testInk
                    height: CelestinaTheme.controlHeightXs
                    reading: ({"volume": 42, "muted": false,
                               "micMuted": false, "micVolume": 70})
                }

                Desktop.BrightnessLevel {
                    ink: testInk
                    height: CelestinaTheme.controlHeightXs
                    outputName: "DP-1"
                    reading: ({"DP-1": 65, "HDMI-A-1": 80})
                    blurAvailable: true
                    // In a cluster, which owns the glass for its members.
                    ownsGlass: false
                }
            }

            Desktop.PanelCluster {
                id: utilityCluster

                blurAvailable: true
                ink: testInk
                spacing: CelestinaTheme.spaceXs

                Desktop.NotificationIndicator {
                    ink: testInk
                    height: CelestinaTheme.controlHeightXs
                    reading: ({"unread": 3, "quiet": false})
                }

                Desktop.PanelActionButton {
                    id: launcherButton

                    ink: testInk
                    blurAvailable: true
                    ownsGlass: false
                    iconName: "app-window"
                    helpText: qsTr("Abrir el buscador de aplicaciones")
                }

                Desktop.PanelActionButton {
                    id: controlCentreButton

                    ink: testInk
                    blurAvailable: true
                    ownsGlass: false
                    iconName: "settings"
                    helpText: qsTr("Abrir el centro de control")
                }

                Desktop.PanelActionButton {
                    id: clipboardButton

                    ink: testInk
                    blurAvailable: true
                    ownsGlass: false
                    iconName: "clipboard-paste"
                    helpText: qsTr("Abrir el historial del portapapeles")
                }

                Desktop.PanelActionButton {
                    id: sessionButton

                    ink: testInk
                    blurAvailable: true
                    ownsGlass: false
                    iconName: "power"
                    helpText: qsTr("Abrir el menú de sesión")
                }

                Desktop.SysMon {
                    id: performanceButton

                    ink: testInk
                    blurAvailable: true
                    ownsGlass: false
                    reading: ({"cpu": 6, "ram": 24})
                }
            }

            Desktop.PhoneStatus {
                ink: testInk
                blurAvailable: true
                connected: true
                battery: 49
                charging: false
            }
        }
    }

    function init() {
        panel.visible = true;
        drawer.items = testCase.session;
        drawer.preferences = [];
        flankTray.preferences = [];
        drawerRequests.clear();
        trayActivations.clear();
        trayItemMenus.clear();
    }

    function cleanup() {
        panel.visible = true;
    }

    // 1. The compact opener does its job: nothing here asks for attention, so
    //    no foreign icon spends space in the bar. The inventory button remains
    //    and carries the count only in its accessible name.
    function test_the_compact_opener_has_no_visible_count() {
        compare(testCase.entriesOf(drawer).length, 0);

        const toggle = findChild(drawer, "celestina-tray-toggle");
        verify(toggle, "the drawer has a toggle");
        verify(toggle.visible, "the toggle is visible");
        verify(toggle.width > 0, "the toggle has width: " + toggle.width);
        // The count is in the control's own name, which is what a screen reader
        // reads; the shell deliberately paints no hover tooltip.
        verify(toggle.helpText.indexOf("4") >= 0, "the toggle names the count: " + toggle.helpText);
        const glyph = findChild(toggle, "celestina-tray-toggle-icon");
        verify(glyph);
        compare(glyph.name, "system-tray");
        compare(glyph.width, CelestinaTheme.iconSm);
        compare(glyph.height, CelestinaTheme.iconSm);
        verify(glyph.width < toggle.width,
               "the inventory glyph stays inside the compact button");
        // The row itself is present, so the toggle can be reached at all.
        verify(drawer.visible, "the tray opener row is visible");

        const count = findChild(drawer, "celestina-tray-count");
        compare(count, null, "the count must not consume visible bar space");
    }

    function test_the_button_requests_a_contextual_menu_at_its_real_rectangle() {
        drawerRequests.clear();
        const toggle = findChild(drawer, "celestina-tray-toggle");
        verify(toggle);
        toggle.click();
        compare(drawerRequests.count, 1);
        const arguments = drawerRequests.signalArguments[0];
        const expected = toggle.mapToGlobal(0, 0);
        compare(arguments[0].x, expected.x);
        compare(arguments[0].y, expected.y);
        compare(arguments[0].width, toggle.width);
        compare(arguments[0].height, toggle.height);
        const anchor = arguments[1];
        compare(anchor, toggle.attachmentAnchorGlobalRectNow());
        compare(anchor.width, 18);
        compare(anchor.height, 18);
        const glyph = findChild(toggle, "celestina-tray-toggle-icon");
        verify(glyph);
        const glyphAt = glyph.mapToGlobal(0, 0);
        compare(anchor.x, glyphAt.x);
        compare(anchor.y, glyphAt.y);
        verify(toggle.isPanelAttachmentSource);
    }

    // An empty tray is not a tray with a zero on it.
    function test_an_empty_tray_shows_nothing_at_all() {
        drawer.items = [];
        verify(!drawer.visible);
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

    // An item that does ask for attention is shown, and only that one.
    function test_the_compact_opener_still_shows_what_asks_for_attention() {
        const urgent = JSON.parse(JSON.stringify(testCase.solaar));
        urgent.status = "attention";
        drawer.items = [testCase.slack, urgent, testCase.applet, testCase.blueman];

        const shown = testCase.entriesOf(drawer);
        compare(shown.length, 1);
        compare(shown[0].modelData.title, "Solaar");

        drawer.items = testCase.session;
    }

    function test_pinned_items_follow_the_opener_and_hidden_items_never_leak() {
        const urgentSolaar = JSON.parse(JSON.stringify(testCase.solaar));
        urgentSolaar.status = "attention";
        drawer.items = [testCase.slack, urgentSolaar, testCase.applet];
        drawer.preferences = [
            {"key": testCase.slack.preferenceKey, "mode": "pinned"},
            {"key": testCase.solaar.preferenceKey, "mode": "hidden"}
        ];
        wait(0);

        const toggle = findChild(drawer, "celestina-tray-toggle");
        const shown = testCase.entriesOf(drawer);
        compare(shown.length, 1);
        compare(shown[0].modelData.service, testCase.slack.service);
        const itemOnDrawer = shown[0].mapToItem(drawer, 0, 0);
        verify(itemOnDrawer.x > toggle.x,
               "a pinned item belongs immediately to the opener's right: item "
               + itemOnDrawer.x + ", opener " + toggle.x);
    }

    function test_attention_does_not_duplicate_a_pin() {
        const urgentPinned = JSON.parse(JSON.stringify(testCase.slack));
        urgentPinned.status = "attention";
        drawer.items = [urgentPinned];
        drawer.preferences = [
            {"key": testCase.slack.preferenceKey, "mode": "pinned"}
        ];

        const shown = testCase.entriesOf(drawer);
        compare(shown.length, 1);
        compare(shown[0].modelData.path, testCase.slack.path);
    }

    function test_missing_and_failed_icons_use_a_fixed_catalogue_fallback() {
        const longUrgent = JSON.parse(JSON.stringify(testCase.solaar));
        longUrgent.status = "attention";
        longUrgent.title = "A tray application with a deliberately long title";
        drawer.items = [longUrgent];
        wait(0);

        let entry = testCase.entriesOf(drawer)[0];
        let fallback = findChild(entry, "celestina-tray-item-fallback-icon");
        verify(fallback);
        verify(fallback.visible);
        compare(fallback.name, "app-window");
        compare(entry.width, CelestinaTheme.iconSm);
        compare(entry.height, CelestinaTheme.iconSm);
        compare(findChild(entry, "celestina-tray-item-title"), null);
        compare(entry.Accessible.name, longUrgent.title);

        const brokenUrgent = JSON.parse(JSON.stringify(testCase.slack));
        brokenUrgent.status = "attention";
        brokenUrgent.title = "Icono roto";
        brokenUrgent.iconSource = "file:///celestina-test/no-such-tray-icon.png";
        drawer.items = [brokenUrgent];
        wait(0);

        entry = testCase.entriesOf(drawer)[0];
        const image = findChild(entry, "celestina-tray-item-image");
        fallback = findChild(entry, "celestina-tray-item-fallback-icon");
        verify(image);
        verify(fallback);
        tryCompare(image, "status", Image.Error);
        tryCompare(fallback, "visible", true);
        compare(fallback.name, "app-window");
        compare(entry.width, CelestinaTheme.iconSm);
        compare(entry.height, CelestinaTheme.iconSm);
        compare(entry.Accessible.name, brokenUrgent.title);
    }

    function test_pinned_and_urgent_items_are_keyboard_operable() {
        panel.visible = false;
        const urgent = JSON.parse(JSON.stringify(testCase.solaar));
        urgent.status = "attention";
        drawer.items = [urgent];
        waitForRendering(drawer);

        const entry = testCase.entriesOf(drawer)[0];
        const focusRing = findChild(entry, "celestina-tray-item-focus");
        verify(entry);
        verify(entry.activeFocusOnTab);
        verify(focusRing);
        entry.forceActiveFocus(Qt.TabFocusReason);
        tryCompare(entry, "activeFocus", true);
        tryCompare(focusRing, "visible", true);

        keyClick(Qt.Key_Return);
        keyClick(Qt.Key_Enter);
        keyClick(Qt.Key_Space);
        compare(trayActivations.count, 3);
        for (let index = 0; index < trayActivations.count; ++index) {
            compare(trayActivations.signalArguments[index][0], urgent.service);
            compare(trayActivations.signalArguments[index][1], urgent.path);
        }

        keyClick(Qt.Key_Menu);
        keyClick(Qt.Key_F10, Qt.ShiftModifier);
        compare(trayItemMenus.count, 2);
        for (let index = 0; index < trayItemMenus.count; ++index) {
            compare(trayItemMenus.signalArguments[index][0], urgent.service);
            compare(trayItemMenus.signalArguments[index][1], urgent.path);
        }
    }

    function test_a_tray_item_menu_press_is_not_silent() {
        // `panel` occupies the same synthetic screen coordinates as `host` and
        // is declared later, so hide that independent fixture while sending a
        // real window event to this drawer.
        panel.visible = false;
        const urgent = JSON.parse(JSON.stringify(testCase.slack));
        urgent.status = "attention";
        drawer.items = [urgent];
        waitForRendering(drawer);
        const entry = testCase.entriesOf(drawer)[0];
        const pointer = findChild(entry, "celestina-tray-item-pointer");
        const feedback = findChild(entry, "celestina-tray-item-feedback");
        verify(pointer);
        verify(feedback);

        mousePress(pointer, pointer.width / 2, pointer.height / 2,
                   Qt.RightButton);
        verify(pointer.pressed);
        tryCompare(feedback, "color", CelestinaTheme.surfaceStrong);
        mouseRelease(pointer, pointer.width / 2, pointer.height / 2,
                     Qt.RightButton);
        verify(!pointer.pressed);
        drawer.items = testCase.session;
        panel.visible = true;
    }

    // 2. The contextual inventory never widens the panel. The real flank still
    //    has room for the launcher and every existing permanent action.
    function test_requesting_the_inventory_does_not_expand_or_clip_the_flank() {
        wait(0);

        verify(rightFlank.width > 0);
        compare(connectivityCluster.spacing, CelestinaTheme.spaceMd);
        compare(levelCluster.spacing, CelestinaTheme.spaceMd);
        compare(utilityCluster.spacing, CelestinaTheme.spaceXs);
        // The clusters paint denser material only; the real Panel supplies one
        // continuous edge-to-edge compositor region behind every cluster.
        compare(testCase.visibleGlassRegions(connectivityCluster).length, 0);
        compare(testCase.visibleGlassRegions(levelCluster).length, 0);
        compare(testCase.visibleGlassRegions(utilityCluster).length, 0);
        verify(!launcherButton.ownsGlass);
        verify(!controlCentreButton.ownsGlass);
        verify(!clipboardButton.ownsGlass);
        verify(!sessionButton.ownsGlass);
        verify(!performanceButton.ownsGlass);
        compare(testCase.entriesOf(flankTray).length, 0);
        verify(rightFlank.contentWidth <= rightFlank.width);

        const before = flankTray.implicitWidth;
        const toggle = findChild(flankTray, "celestina-tray-toggle");
        verify(toggle);
        toggle.click();
        wait(0);
        compare(flankTray.implicitWidth, before);
        compare(testCase.entriesOf(flankTray).length, 0);
        verify(rightFlank.contentWidth <= rightFlank.width);
    }
}
