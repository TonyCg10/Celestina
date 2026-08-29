import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

// The shell-owned tray inventory stays one real contextual menu while its
// fixed card switches between two icon-only views. TrayMenu remains the
// independent child that renders one foreign application's D-Bus menu.
TestCase {
    id: testCase

    name: "TrayItemsMenu"

    readonly property var slack: ({
        "service": ":1.83",
        "path": "/org/chromium/StatusNotifierItem/1",
        "preferenceKey": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "title": "Slack",
        "status": "active",
        "hasMenu": true,
        "iconSource": CelestinaIcons.source("app-window")
    })
    readonly property var solaar: ({
        "service": ":1.32",
        "path": "/org/ayatana/NotificationItem/indicator_solaar",
        "preferenceKey": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "title": "Solaar",
        "status": "attention",
        "hasMenu": true,
        "iconSource": ""
    })

    QtObject {
        id: fakeTray

        property var items: [testCase.slack, testCase.solaar]
        signal changed()

        function publish(next) {
            fakeTray.items = next;
            fakeTray.changed();
        }
    }

    QtObject {
        id: fakeLedger

        property int revision: 0
        property var states: ({})

        function stateOf(provider, target) {
            return fakeLedger.states[target] !== undefined
                   ? fakeLedger.states[target] : ({});
        }

        function publishState(target, state, cause) {
            const next = Object.assign({}, fakeLedger.states);
            next[target] = {"state": state, "cause": cause || ""};
            fakeLedger.states = next;
            fakeLedger.revision = fakeLedger.revision + 1;
        }

        function send(provider, verb, options, target, policy) {
            fakeSource.sent.push({
                "provider": provider,
                "verb": verb,
                "options": options,
                "target": target,
                "policy": policy
            });
            return "1";
        }
    }

    QtObject {
        id: fakeSource

        property int revision: 0
        property var providers: ({"settings": {"trayItems": []}})
        property var requests: fakeLedger
        property var sent: []

        function publishPreferences(rows) {
            fakeSource.providers = {"settings": {"trayItems": rows}};
            fakeSource.revision = fakeSource.revision + 1;
        }
    }

    Desktop.TrayItemsMenu {
        id: trayMenu

        visible: true
        outputName: "test-output"
        reducedMotion: true
        traySource: fakeTray
        providerSource: fakeSource
    }

    SignalSpy {
        id: activations
        target: trayMenu
        signalName: "activated"
    }

    SignalSpy {
        id: secondaryActivations
        target: trayMenu
        signalName: "secondaryActivated"
    }

    SignalSpy {
        id: itemMenus
        target: trayMenu
        signalName: "itemMenuRequested"
    }

    SignalSpy {
        id: menuClosed
        target: trayMenu.menu
        signalName: "closed"
    }

    function grid() {
        return findChild(trayMenu, "celestina-tray-icon-grid");
    }

    function card() {
        return findChild(trayMenu, "celestina-tray-inventory-card");
    }

    function visibleSelector() {
        return findChild(trayMenu, "celestina-tray-visible-selector");
    }

    function hiddenSelector() {
        return findChild(trayMenu, "celestina-tray-hidden-selector");
    }

    function tileAt(index) {
        const view = testCase.grid();
        return view ? view.itemAtIndex(index) : null;
    }

    function init() {
        fakeTray.publish([testCase.slack, testCase.solaar]);
        fakeSource.sent = [];
        fakeLedger.states = ({});
        fakeLedger.revision = fakeLedger.revision + 1;
        fakeSource.publishPreferences([]);
        trayMenu.inventoryMode = "visible";
        trayMenu.width = 720;
        trayMenu.height = 540;
        trayMenu.menuX = 20;
        trayMenu.menuY = 46;
        trayMenu.maximumContentHeight = 494;
        if (trayMenu.menu.visible || trayMenu.menu.opened) {
            menuClosed.clear();
            trayMenu.menu.close();
            tryCompare(menuClosed, "count", 1);
        }
        trayMenu.menu.open();
        tryCompare(trayMenu.menu, "opened", true);
        tryCompare(trayMenu.menu, "count", 2);
        tryVerify(function() {
            return testCase.grid() && testCase.grid().count === 2
                   && testCase.tileAt(0) && testCase.tileAt(1);
        });
        activations.clear();
        secondaryActivations.clear();
        itemMenus.clear();
    }

    function test_it_is_one_fixed_card_with_a_section_mode_switch_and_icon_grid() {
        compare(trayMenu.menu.modal, false);
        compare(trayMenu.headerBodyGap, CelestinaTheme.spaceMd);
        compare(trayMenu.menu.count, 2);
        verify(findChild(trayMenu, "celestina-menu-header"));
        // SIMPLE-1: the sections are the frosted cards; each publishes its
        // region once the fade has landed.
        tryVerify(function() { return trayMenu.glassRegions.length >= 1; });
        const section = findChild(trayMenu, "celestina-menu-section");
        verify(section);
        const tint = findChild(section, "celestina-panel-tint");
        verify(tint);
        fuzzyCompare(tint.color.r, CelestinaTheme.elevated.r, 0.01);
        fuzzyCompare(tint.color.a, 0.55, 0.01);

        const header = trayMenu.menu.itemAt(0);
        const inventory = trayMenu.menu.itemAt(1);
        verify(header);
        verify(inventory);
        compare(header.iconName, "system-tray");
        compare(header.fallbackIcon, "view-grid");
        compare(header.headerTrailingGap, CelestinaTheme.spaceMd);
        compare(inventory.height, trayMenu.inventoryCardHeight);

        const heading = findChild(trayMenu,
                                  "celestina-tray-applications-heading");
        const visibleMode = testCase.visibleSelector();
        const hiddenMode = testCase.hiddenSelector();
        verify(heading);
        verify(visibleMode);
        verify(hiddenMode);
        compare(heading.text, qsTr("Aplicaciones"));
        compare(visibleMode.text, "");
        compare(hiddenMode.text, "");
        compare(visibleMode.iconName, "eye");
        compare(hiddenMode.iconName, "eye-off");
        compare(visibleMode.width, CelestinaTheme.controlHeightXs);
        compare(hiddenMode.width, CelestinaTheme.controlHeightXs);
        compare(visibleMode.Accessible.name,
                qsTr("Aplicaciones visibles (2)"));
        compare(hiddenMode.Accessible.name,
                qsTr("Aplicaciones ocultas (0)"));
        compare(visibleMode.checked, true);
        compare(hiddenMode.checked, false);
        compare(visibleMode.activeFocusOnTab, true);
        compare(hiddenMode.activeFocusOnTab, true);

        const view = testCase.grid();
        compare(view.count, 2);
        compare(view.cellHeight, trayMenu.tileCellHeight);
        compare(view.Accessible.name, qsTr("Aplicaciones visibles"));

        // Producer titles are accessibility identity, never painted tile text.
        const slackTile = testCase.tileAt(0);
        const solaarTile = testCase.tileAt(1);
        compare(slackTile.text, "");
        compare(slackTile.Accessible.name, "Slack");
        compare(slackTile.iconSource.toString(), testCase.slack.iconSource);
        const slackIcon = findChild(
                    slackTile, "celestina-tray-tile-external-icon");
        verify(slackIcon);
        compare(slackIcon.width, trayMenu.inventoryIconSize);
        verify(slackIcon.width > CelestinaTheme.iconMd);
        compare(solaarTile.text, "");
        compare(solaarTile.Accessible.name, "Solaar");
        const solaarFallback = findChild(
                    solaarTile, "celestina-tray-tile-fallback-icon");
        verify(solaarFallback);
        compare(solaarFallback.width, trayMenu.inventoryIconSize);
        verify(solaarTile.Accessible.description.indexOf(qsTr("requiere atención"))
               >= 0);
        const pin = findChild(slackTile, "celestina-tray-tile-pin");
        const visibility = findChild(slackTile,
                                     "celestina-tray-tile-visibility");
        verify(pin);
        verify(visibility);
        compare(pin.activeFocusOnTab, true);
        compare(visibility.activeFocusOnTab, true);
        compare(slackTile.leftPadding, 0);
        compare(slackTile.rightPadding, 0);
        compare(slackTile.availableWidth, slackTile.width);
        verify(pin.x + pin.width <= visibility.x);
    }

    function test_a_failed_foreign_icon_uses_the_fixed_catalogue_fallback() {
        const broken = Object.assign({}, testCase.slack, {
            "iconSource": "file:///celestina-test/no-such-menu-icon.png"
        });
        fakeTray.publish([broken, testCase.solaar]);

        tryVerify(function() {
            return testCase.tileAt(0) !== null;
        });
        const brokenTile = testCase.tileAt(0);
        const image = findChild(brokenTile,
                                "celestina-tray-tile-external-icon");
        const fallback = findChild(brokenTile,
                                   "celestina-tray-tile-fallback-icon");
        verify(image);
        verify(fallback);
        tryCompare(image, "status", Image.Error);
        compare(brokenTile.externalIconFailed, true);
        compare(brokenTile.externalIconReady, false);
        tryCompare(fallback, "visible", true);
        compare(fallback.name, "app-window");
        compare(brokenTile.text, "");
    }

    function test_primary_secondary_and_child_menu_keep_exact_identity() {
        const slackTile = testCase.tileAt(0);
        verify(slackTile);

        mouseClick(slackTile, slackTile.width / 2, slackTile.height / 3,
                   Qt.MiddleButton);
        compare(secondaryActivations.count, 1);
        compare(secondaryActivations.signalArguments[0][0],
                testCase.slack.service);
        compare(secondaryActivations.signalArguments[0][1],
                testCase.slack.path);
        verify(secondaryActivations.signalArguments[0][2] >= 0);
        verify(secondaryActivations.signalArguments[0][3] >= 0);

        mouseClick(slackTile, slackTile.width / 2, slackTile.height / 3,
                   Qt.RightButton);
        compare(itemMenus.count, 1);
        compare(itemMenus.signalArguments[0][0], testCase.slack.service);
        compare(itemMenus.signalArguments[0][1], testCase.slack.path);
        compare(itemMenus.signalArguments[0][2], testCase.slack.title);
        verify(itemMenus.signalArguments[0][3] >= 0);
        verify(itemMenus.signalArguments[0][4] >= 0);
        verify(itemMenus.signalArguments[0][5] > 0);
        verify(itemMenus.signalArguments[0][6] > 0);
        // The request is for the independent child carrier; the inventory does
        // not dismiss itself first.
        compare(trayMenu.menu.visible, true);

        slackTile.click();
        compare(activations.count, 1);
        compare(activations.signalArguments[0][0], testCase.slack.service);
        compare(activations.signalArguments[0][1], testCase.slack.path);
    }

    function test_grid_keyboard_keeps_primary_secondary_and_menu_routes() {
        const view = testCase.grid();
        trayMenu.requestActivate();
        tryCompare(trayMenu, "active", true);
        view.currentIndex = 0;
        view.forceActiveFocus();
        tryCompare(view, "activeFocus", true);

        keyClick(Qt.Key_Enter, Qt.ControlModifier);
        compare(secondaryActivations.count, 1);
        compare(secondaryActivations.signalArguments[0][0],
                testCase.slack.service);
        compare(secondaryActivations.signalArguments[0][1],
                testCase.slack.path);

        keyClick(Qt.Key_F10, Qt.ShiftModifier);
        compare(itemMenus.count, 1);
        compare(itemMenus.signalArguments[0][0], testCase.slack.service);
        compare(itemMenus.signalArguments[0][1], testCase.slack.path);
        compare(itemMenus.signalArguments[0][2], testCase.slack.title);
        compare(trayMenu.menu.visible, true);

        keyClick(Qt.Key_Right);
        tryCompare(view, "currentIndex", 1);
        keyClick(Qt.Key_Return);
        compare(activations.count, 1);
        compare(activations.signalArguments[0][0], testCase.solaar.service);
        compare(activations.signalArguments[0][1], testCase.solaar.path);
    }

    function test_the_open_grid_follows_the_live_inventory_without_resizing() {
        const naturalHeight = trayMenu.contentHeight;
        const visibleHeight = trayMenu.cardHeight;
        const fixedTop = trayMenu.cardY;
        compare(testCase.grid().count, 2);

        fakeTray.publish([testCase.solaar]);
        tryCompare(testCase.grid(), "count", 1);
        tryVerify(function() {
            return testCase.tileAt(0) !== null;
        });
        compare(testCase.tileAt(0).Accessible.name, "Solaar");
        compare(trayMenu.menu.count, 2);
        compare(trayMenu.contentHeight, naturalHeight);
        compare(trayMenu.cardHeight, visibleHeight);
        compare(trayMenu.cardY, fixedTop);
    }

    function test_pin_hide_and_restore_wait_for_durable_truth_in_stable_modes() {
        const naturalHeight = trayMenu.contentHeight;
        const visibleHeight = trayMenu.cardHeight;
        const fixedTop = trayMenu.cardY;
        const slackTile = testCase.tileAt(0);
        const pin = findChild(slackTile, "celestina-tray-tile-pin");
        const visibility = findChild(slackTile,
                                     "celestina-tray-tile-visibility");
        verify(pin);
        verify(visibility);

        pin.forceActiveFocus();
        tryCompare(pin, "activeFocus", true);
        mouseClick(pin, pin.width / 2, pin.height / 2, Qt.LeftButton);
        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].provider, "settings");
        compare(fakeSource.sent[0].verb, "tray-item-mode");
        compare(fakeSource.sent[0].options.key,
                testCase.slack.preferenceKey);
        compare(fakeSource.sent[0].options.mode, "pinned");
        compare(fakeSource.sent[0].policy, "immediate");
        // No optimistic movement or selected state.
        compare(testCase.grid().count, 2);
        compare(pin.role, CelestinaButton.Ghost);
        compare(activations.count, 0);

        visibility.forceActiveFocus();
        tryCompare(visibility, "activeFocus", true);
        mouseClick(visibility, visibility.width / 2,
                   visibility.height / 2, Qt.LeftButton);
        compare(fakeSource.sent.length, 2);
        compare(fakeSource.sent[1].options.mode, "hidden");
        compare(testCase.grid().count, 2);
        compare(activations.count, 0);

        fakeSource.publishPreferences([
            {"key": testCase.slack.preferenceKey, "mode": "hidden"}
        ]);
        tryCompare(testCase.grid(), "count", 1);
        tryCompare(testCase.grid(), "activeFocus", true);
        tryCompare(testCase.hiddenSelector().Accessible, "name",
                   qsTr("Aplicaciones ocultas (1)"));
        compare(trayMenu.contentHeight, naturalHeight);
        compare(trayMenu.cardHeight, visibleHeight);
        compare(trayMenu.cardY, fixedTop);

        testCase.hiddenSelector().click();
        tryCompare(trayMenu, "inventoryMode", "hidden");
        tryCompare(testCase.grid(), "count", 1);
        tryVerify(function() {
            return testCase.tileAt(0) !== null;
        });
        const hiddenTile = testCase.tileAt(0);
        compare(hiddenTile.text, "");
        compare(hiddenTile.Accessible.name, "Slack");
        compare(findChild(hiddenTile, "celestina-tray-tile-pin").visible,
                false);
        const restore = findChild(hiddenTile,
                                  "celestina-tray-tile-visibility");
        compare(restore.iconName, "eye");
        compare(restore.Accessible.description, "Slack");
        compare(trayMenu.contentHeight, naturalHeight);
        compare(trayMenu.cardHeight, visibleHeight);
        compare(trayMenu.cardY, fixedTop);

        restore.forceActiveFocus();
        tryCompare(restore, "activeFocus", true);
        mouseClick(restore, restore.width / 2, restore.height / 2,
                   Qt.LeftButton);
        compare(fakeSource.sent.length, 3);
        compare(fakeSource.sent[2].options.mode, "visible");
        compare(activations.count, 0);

        fakeSource.publishPreferences([]);
        tryCompare(testCase.grid(), "count", 0);
        tryCompare(testCase.hiddenSelector(), "activeFocus", true);
        compare(trayMenu.inventoryMode, "hidden");
        compare(testCase.card().height, trayMenu.inventoryCardHeight);
        compare(trayMenu.contentHeight, naturalHeight);
        compare(trayMenu.cardHeight, visibleHeight);
        compare(trayMenu.cardY, fixedTop);

        testCase.visibleSelector().click();
        tryCompare(testCase.grid(), "count", 2);
        compare(trayMenu.contentHeight, naturalHeight);
        compare(trayMenu.cardY, fixedTop);
    }

    function test_escape_from_a_nested_tile_action_closes_the_real_menu() {
        const pin = findChild(testCase.tileAt(0),
                              "celestina-tray-tile-pin");
        verify(pin);
        trayMenu.requestActivate();
        tryCompare(trayMenu, "active", true);
        pin.forceActiveFocus();
        tryCompare(pin, "activeFocus", true);

        menuClosed.clear();
        keyClick(Qt.Key_Escape);

        tryCompare(menuClosed, "count", 1);
        compare(trayMenu.menu.visible, false);
    }

    function test_unrelated_inventory_updates_do_not_consume_focus_restore() {
        const slackTile = testCase.tileAt(0);
        const visibility = findChild(slackTile,
                                     "celestina-tray-tile-visibility");
        const newcomer = Object.assign({}, testCase.solaar, {
            "service": ":1.99",
            "path": "/item/newcomer",
            "preferenceKey": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "title": "Nueva"
        });

        visibility.forceActiveFocus();
        tryCompare(visibility, "activeFocus", true);
        mouseClick(visibility, visibility.width / 2,
                   visibility.height / 2, Qt.LeftButton);
        compare(trayMenu.pendingFocusRestoreKey,
                testCase.slack.preferenceKey);
        compare(trayMenu.pendingFocusRestoreMode, "hidden");

        // A different StatusNotifierItem arriving rebuilds displayedItems but
        // does not confirm the requested settings transition.
        fakeTray.publish([testCase.slack, testCase.solaar, newcomer]);
        tryCompare(testCase.grid(), "count", 3);
        compare(trayMenu.pendingFocusRestoreKey,
                testCase.slack.preferenceKey);
        compare(trayMenu.pendingFocusRestoreMode, "hidden");

        fakeSource.publishPreferences([
            {"key": testCase.slack.preferenceKey, "mode": "hidden"}
        ]);
        tryCompare(testCase.grid(), "count", 2);
        tryCompare(testCase.grid(), "activeFocus", true);
        tryCompare(trayMenu, "pendingFocusRestoreKey", "");
        compare(trayMenu.pendingFocusRestoreMode, "");
    }

    function test_large_modes_scroll_without_moving_or_resizing_the_card() {
        const many = [];
        const preferences = [];
        for (let index = 0; index < 14; ++index) {
            const item = Object.assign({}, testCase.slack, {
                "service": ":1." + (100 + index),
                "path": "/item/" + index,
                "preferenceKey": "hidden-key-" + index,
                "title": "Oculta " + index,
                "iconSource": ""
            });
            many.push(item);
            preferences.push({"key": item.preferenceKey, "mode": "hidden"});
        }

        trayMenu.height = 260;
        trayMenu.menuY = 46;
        trayMenu.maximumContentHeight = 214;
        fakeTray.publish(many);
        fakeSource.publishPreferences(preferences);
        tryCompare(testCase.grid(), "count", 0);
        const naturalHeight = trayMenu.contentHeight;
        const fixedTop = trayMenu.cardY;
        compare(fixedTop, 46);
        compare(trayMenu.cardHeight, 214);

        testCase.hiddenSelector().click();
        tryCompare(trayMenu, "inventoryMode", "hidden");
        tryCompare(testCase.grid(), "count", 14);
        compare(trayMenu.contentHeight, naturalHeight);
        compare(trayMenu.cardHeight, 214);
        compare(trayMenu.cardY, fixedTop);

        const view = testCase.grid();
        verify(view.contentHeight > view.height);
        trayMenu.requestActivate();
        tryCompare(trayMenu, "active", true);
        view.currentIndex = 13;
        view.positionViewAtIndex(13, GridView.Contain);
        view.forceActiveFocus();
        tryVerify(function() {
            return view.currentItem !== null && view.contentY > 0;
        });
        compare(view.currentItem.Accessible.name, "Oculta 13");
        keyClick(Qt.Key_Return);
        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].options.key, "hidden-key-13");
        compare(fakeSource.sent[0].options.mode, "visible");
        compare(trayMenu.contentHeight, naturalHeight);
        compare(trayMenu.cardHeight, 214);
        compare(trayMenu.cardY, fixedTop);
    }

    function test_a_persistence_failure_is_visible_and_retryable() {
        const slackTile = testCase.tileAt(0);
        const pin = findChild(slackTile, "celestina-tray-tile-pin");
        const target = "tray-item-mode:" + testCase.slack.preferenceKey;

        fakeLedger.publishState(target, "pending", "");
        tryCompare(slackTile, "waiting", true);
        compare(pin.enabled, false);
        verify(slackTile.Accessible.description.indexOf("guardando") >= 0);

        fakeLedger.publishState(target, "failed", "reported");
        tryCompare(slackTile, "failed", true);
        compare(pin.enabled, true);
        verify(slackTile.Accessible.description.indexOf("no se pudo guardar")
               >= 0);

        pin.click();
        compare(fakeSource.sent.length, 1);
        compare(fakeSource.sent[0].target, target);
    }
}
