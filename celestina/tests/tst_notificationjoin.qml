import CelestinaStyle
import QtQuick
import QtQuick.Controls
import QtTest
import "../qml" as Desktop

// Actions arrive beside their notifications, never inside them: the host takes
// one level of structure, and a row carrying its own list is a frame it
// refuses — which on a live session emptied the whole bar. These check the
// other half of that split: that the surfaces still put each action back with
// the notification that offers it.
TestCase {
    id: testCase

    name: "NotificationJoin"

    QtObject {
        id: fakeSource

        property var providers: ({})
        property var sent: []

        function sendCommand(provider, verb, options) {
            fakeSource.sent.push({"verb": verb, "options": options});
            return 1;
        }
    }

    function toast(id, summary, count) {
        return {
            "id": id, "app": "Magnetita", "summary": summary, "body": "a body",
            "urgency": "normal", "read": false, "actionCount": count
        };
    }

    QtObject {
        id: lifecycleProbe

        property int departures: 0
    }

    Component {
        id: lifecycleStackComponent

        Desktop.ToastStack {}
    }

    property var transientStack: null

    function routeRows() {
        return [
            {"tag": "attached-top-right", "route": "attached"},
            {"tag": "bottom-centre", "route": "bottom"},
            {"tag": "corner", "route": "corner"}
        ];
    }

    function createLifecycleStack(route, initialToasts) {
        const properties = {
            "toasts": initialToasts,
            "actions": [],
            "providerSource": fakeSource,
            "reducedMotion": false,
            "surfaceWidth": 380,
            "surfaceHeight": 0
        };
        if (route === "attached") {
            properties.anchoredFromPanel = true;
            properties.openerRect = Qt.rect(420, -35, 30, 30);
            properties.attachmentAnchorRect = Qt.rect(426, -29, 18, 18);
            properties.attachmentStartY = 0;
            properties.surfaceOriginX = 0;
            properties.surfaceWidth = 620;
            properties.surfaceHeight = 280;
        } else if (route === "bottom") {
            properties.entersFromBottom = true;
        }

        const created = lifecycleStackComponent.createObject(null, properties);
        verify(created !== null);
        lifecycleProbe.departures = 0;
        created.departureFinished.connect(function() {
            lifecycleProbe.departures += 1;
        });
        transientStack = created;
        return created;
    }

    function fieldFor(window) {
        const field = findChild(window.contentItem,
                                "celestina-soft-menu-field");
        verify(field !== null);
        return field;
    }

    function toastSectionCount(window) {
        let count = 0;
        function collect(item) {
            for (let index = 0; index < item.children.length; ++index) {
                const child = item.children[index];
                if (child.objectName === "celestina-menu-section")
                    count += 1;
                collect(child);
            }
        }
        collect(window.contentItem);
        return count;
    }

    function settleLifecycleStack(window, field) {
        window.show();
        tryCompare(window, "visible", true);
        // Make the presentation edge deterministic in the offscreen runner;
        // production reaches this state through the window's frame swap.
        field.surfacePresented = true;
        field.revealNow();
        wait(CelestinaTheme.motionNormal * 2 + 40);
        tryVerify(function() {
            return field.revealed && window.glassRegions.length === 1;
        });
    }

    function cleanup() {
        if (transientStack === null)
            return;
        transientStack.hide();
        transientStack.destroy();
        transientStack = null;
    }

    Desktop.ToastStack {
        id: stack

        toasts: [testCase.toast(1, "Uno", 2), testCase.toast(2, "Dos", 0)]
        actions: [
            {"notification": 1, "key": "open", "label": "Abrir"},
            {"notification": 1, "key": "mute", "label": "Silenciar"},
            {"notification": 9, "key": "stray", "label": "De nadie"}
        ]
        providerSource: fakeSource
        reducedMotion: false
    }

    Desktop.NotificationCenter {
        id: centre

        providerSource: fakeSource
        reducedMotion: false
    }

    function test_a_toast_gets_exactly_the_actions_it_offers() {
        const mine = stack.actionsFor(1);
        compare(mine.length, 2);
        compare(mine[0].key, "open");
        compare(mine[1].label, "Silenciar");
    }

    function test_a_toast_without_actions_gets_none() {
        compare(stack.actionsFor(2).length, 0);
    }

    function test_an_action_for_another_notification_is_never_borrowed() {
        // The stray row names notification 9, which is not on screen. It must
        // not attach itself to whatever is.
        compare(stack.actionsFor(1).length, 2);
        compare(stack.actionsFor(2).length, 0);
        compare(stack.actionsFor(9).length, 1);
    }

    function test_the_centre_joins_from_the_same_sibling_list() {
        fakeSource.providers = {
            "notifications": {
                "unread": 1, "quiet": false, "historyCap": 50, "historyTruncated": false,
                "toasts": [testCase.toast(5, "Cinco", 1)],
                "history": [],
                "actions": [{"notification": 5, "key": "reply", "label": "Responder"}]
            }
        };

        const offered = centre.actionsFor(5);
        compare(offered.length, 1);
        compare(offered[0].key, "reply");
    }

    function test_a_server_that_is_not_ours_offers_nothing() {
        // Another program owns the notification name: there is no payload at
        // all, and asking for actions must not throw.
        fakeSource.providers = {};
        compare(centre.actionsFor(5).length, 0);
    }

    function test_toast_dismiss_keeps_its_name_without_a_hover_tooltip() {
        stack.show();
        tryCompare(stack, "visible", true);
        tryVerify(function() {
            return findChild(stack.contentItem,
                             "celestina-toast-dismiss") !== null;
        });
        const dismiss = findChild(stack.contentItem,
                                  "celestina-toast-dismiss");
        verify(dismiss);
        verify(dismiss.helpText.length > 0);
        compare(dismiss.Accessible.name, dismiss.helpText);
        mouseMove(dismiss, dismiss.width / 2, dismiss.height / 2);
        tryCompare(dismiss, "hovered", true);
        compare(dismiss.ToolTip.text, "");
        compare(dismiss.ToolTip.visible, false);
        stack.hide();
    }

    // The stack is one block of the shell's glass: a single veil that grows
    // with the column, publishing one region, while each notification keeps
    // a denser section of its own inside it. It used to be one field per
    // toast — a pile of independent blocks — which the author rejected.
    function test_the_stack_is_one_glass_block_with_a_section_per_toast() {
        stack.show();
        tryCompare(stack, "visible", true);
        tryVerify(function() {
            return stack.glassRegions.length === 1;
        });
        compare(stack.glassRects.length, 1);

        const body = findChild(stack.contentItem, "celestina-menu-body-tint");
        verify(body);
        compare(body.backdropMode, GlassSurface.ExternalBackdrop);
        compare(body.captureActive, false);
        compare(body.materialRole, GlassSurface.ContextualVeil);
        compare(body.elevation, 0);

        const sections = [];
        function collectSections(item) {
            for (let index = 0; index < item.children.length; ++index) {
                const child = item.children[index];
                if (child.objectName === "celestina-menu-section")
                    sections.push(child);
                collectSections(child);
            }
        }
        collectSections(stack.contentItem);
        compare(sections.length, stack.toasts.length);
        compare(sections[0].materialRole, GlassSurface.ContentSurface);
        verify(body.materialStrength < sections[0].materialStrength);
        stack.hide();
    }

    // Attached, the block itself grips the bar — the membrane is one drop
    // out of the bell — and every notification rides inside that one field.
    function test_the_single_block_grips_the_bar() {
        stack.anchoredFromPanel = true;
        stack.openerRect = Qt.rect(1700, 5, 30, 30);
        stack.attachmentAnchorRect = Qt.rect(1706, 11, 18, 18);
        stack.attachmentStartY = 40;
        stack.surfaceOriginX = 1300;
        stack.surfaceWidth = 620;
        stack.surfaceHeight = 400;

        const fields = [];
        function collect(item) {
            for (let index = 0; index < item.children.length; ++index) {
                const child = item.children[index];
                if (child.objectName === "celestina-soft-menu-field")
                    fields.push(child);
                collect(child);
            }
        }
        collect(stack.contentItem);
        compare(fields.length, 1);
        verify(fields[0].attachedToTop);
        // Tall enough for both notifications: the block, not its first row,
        // is what the membrane hangs.
        verify(fields[0].height > 0);

        stack.anchoredFromPanel = false;
        stack.attachmentStartY = -1;
        stack.surfaceOriginX = 0;
        stack.surfaceWidth = stack.cardWidth;
        stack.surfaceHeight = 0;
    }

    function test_bottom_entry_waits_for_the_reveal_gate() {
        const window = createLifecycleStack(
            "bottom", [testCase.toast(20, "Gate", 0)]);
        const field = fieldFor(window);

        // The queued reveal is deliberately refused. A ride tied to model
        // insertion instead of presentation would still advance underneath.
        field.retiring = true;
        compare(field.revealed, false);
        compare(field.blockEntryProgress, 0);
        const heldRide = field.transform[0].y;
        verify(heldRide > 0);

        wait(CelestinaTheme.motionFast);
        compare(field.revealed, false);
        compare(field.blockEntryProgress, 0);
        fuzzyCompare(field.transform[0].y, heldRide, 0.01);
    }

    function test_departure_keeps_the_last_block_until_one_finish_data() {
        return routeRows();
    }

    function test_departure_keeps_the_last_block_until_one_finish(data) {
        const window = createLifecycleStack(
            data.route, [testCase.toast(30, "Leaving", 0)]);
        const field = fieldFor(window);
        settleLifecycleStack(window, field);
        const settledWidth = window.glassRegions[0].rect.width;

        window.toasts = [];
        compare(field.departing, true);
        compare(field.visible, true);
        compare(lifecycleProbe.departures, 0);
        compare(window.glassRegions.length, 1);

        wait(70);
        verify(field.opacity > 0 && field.opacity < 1);
        verify(field.scale > 0.88 && field.scale < 1);
        compare(field.visible, true);
        compare(lifecycleProbe.departures, 0);
        compare(window.glassRegions.length, 1);
        // The compositor footprint must be the currently shrinking block,
        // not the settled rectangle cached before departure began.
        verify(Math.abs(window.glassRegions[0].rect.width
                        - settledWidth * field.scale) < 2);

        tryCompare(lifecycleProbe, "departures", 1);
        compare(field.visible, false);
        compare(field.revealed, false);
        tryVerify(function() {
            return window.glassRegions.length === 0;
        });
    }

    function test_reentry_reverses_the_same_block_data() {
        return routeRows();
    }

    function test_reentry_reverses_the_same_block(data) {
        const window = createLifecycleStack(
            data.route, [testCase.toast(40, "First", 0)]);
        const field = fieldFor(window);
        settleLifecycleStack(window, field);

        window.toasts = [];
        wait(70);
        verify(field.departing);
        verify(field.opacity < 1);
        window.toasts = [testCase.toast(41, "Replacement", 0)];

        compare(field.departing, false);
        compare(field.visible, true);
        compare(lifecycleProbe.departures, 0);
        wait(CelestinaTheme.motionNormal + 60);
        compare(lifecycleProbe.departures, 0);
        fuzzyCompare(field.opacity, 1, 0.01);
        fuzzyCompare(field.scale, 1, 0.01);
        verify(field.revealed);
        compare(window.glassRegions.length, 1);
    }

    function test_full_departure_supersedes_an_armed_row_sweep() {
        const first = testCase.toast(60, "First", 0);
        const second = testCase.toast(61, "Second", 0);
        const window = createLifecycleStack("corner", [first, second]);
        const field = fieldFor(window);
        settleLifecycleStack(window, field);
        compare(toastSectionCount(window), 2);

        // Arm the individual row clock, then empty the server list while that
        // first section is still folding away.
        window.toasts = [second];
        const firstCard = findChild(
            window.contentItem, "celestina-toast-card-60");
        verify(firstCard !== null);
        verify(firstCard.leaving);
        wait(70);
        verify(firstCard.opacity > 0 && firstCard.opacity < 1);

        window.toasts = [];
        compare(field.departing, true);
        compare(firstCard.leaving, false);
        compare(toastSectionCount(window), 2);

        // Past the abandoned rowSweep deadline, but before the newer block
        // deadline: both sections must still be carried by the departing
        // field. The old bug removed the first one in this interval.
        wait(CelestinaTheme.motionNormal - 40);
        compare(lifecycleProbe.departures, 0);
        compare(field.visible, true);
        compare(toastSectionCount(window), 2);

        tryCompare(lifecycleProbe, "departures", 1);
        compare(field.visible, false);
        compare(toastSectionCount(window), 0);
    }

    function test_a_finished_bottom_block_gets_a_fresh_reveal() {
        const window = createLifecycleStack(
            "bottom", [testCase.toast(50, "Old", 0)]);
        const field = fieldFor(window);
        settleLifecycleStack(window, field);

        window.toasts = [];
        tryCompare(lifecycleProbe, "departures", 1);
        compare(field.revealed, false);
        compare(field.blockEntryProgress, 0);

        lifecycleProbe.departures = 0;
        window.toasts = [testCase.toast(51, "Fresh", 0)];
        compare(field.blockEntryProgress, 0);
        verify(field.transform[0].y > 0);
        tryVerify(function() {
            return field.revealed
                   && field.blockEntryProgress > 0
                   && field.blockEntryProgress < 1
                   && window.glassRegions.length === 1;
        });
        const rideDuring = field.transform[0].y;
        const glassDuring = window.glassRegions[0].rect.y;
        wait(40);
        verify(field.transform[0].y < rideDuring);
        verify(window.glassRegions[0].rect.y < glassDuring);
        tryCompare(field, "blockEntryProgress", 1);
        compare(lifecycleProbe.departures, 0);
        compare(window.glassRegions.length, 1);
    }
}
