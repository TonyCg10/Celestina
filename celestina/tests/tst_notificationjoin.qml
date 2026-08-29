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
                if (child.objectName === "celestina-menu-section"
                    && child.visible)
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
        // SIMPLE-1: the surface is a solid card and publishes no glass, so
        // settling is the reveal alone.
        tryVerify(function() {
            return field.revealed;
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

    // SIMPLE-1: the stack is one frosted dark card that grows with the
    // column, with a section per notification inside it — one settled frost
    // region once its fade has landed, never more.
    function test_the_stack_is_one_solid_block_with_a_section_per_toast() {
        stack.show();
        tryCompare(stack, "visible", true);

        // The sections are the visible cards now: one frost region each,
        // published once the block's fade has landed.
        const tint = findChild(stack.contentItem,
                               "celestina-panel-tint");
        verify(tint);
        fuzzyCompare(tint.color.r, CelestinaTheme.elevated.r, 0.01);
        fuzzyCompare(tint.color.a, 0.55, 0.01);
        tryVerify(function() {
            return stack.glassRegions.length === stack.toasts.length;
        });

        const sections = [];
        function collectSections(item) {
            for (let index = 0; index < item.children.length; ++index) {
                const child = item.children[index];
                if (child.objectName === "celestina-menu-section"
                    && child.visible)
                    sections.push(child);
                collectSections(child);
            }
        }
        collectSections(stack.contentItem);
        compare(sections.length, stack.toasts.length);
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

    // SIMPLE-1: the ride is gone; what the gate holds now is the paint. A
    // retiring field refuses its reveal and stays at opacity zero.
    function test_bottom_entry_waits_for_the_reveal_gate() {
        const window = createLifecycleStack(
            "bottom", [testCase.toast(20, "Gate", 0)]);
        const field = fieldFor(window);

        field.retiring = true;
        field.reveal();
        compare(field.revealed, false);
        compare(field.presentationOpacity, 0);

        wait(CelestinaTheme.motionFast);
        compare(field.revealed, false);
        compare(field.presentationOpacity, 0);
        compare(field.transform[0].y, 0);
    }

    function test_departure_keeps_the_last_block_until_one_finish_data() {
        return routeRows();
    }

    function test_departure_keeps_the_last_block_until_one_finish(data) {
        const window = createLifecycleStack(
            data.route, [testCase.toast(30, "Leaving", 0)]);
        const field = fieldFor(window);
        settleLifecycleStack(window, field);

        window.toasts = [];
        compare(field.departing, true);
        compare(field.visible, true);
        compare(lifecycleProbe.departures, 0);

        // SIMPLE-1: leaving is the one fade, mid-flight at half the beat.
        wait(70);
        verify(field.opacity > 0 && field.opacity < 1);
        compare(field.scale, 1);
        compare(field.visible, true);
        compare(lifecycleProbe.departures, 0);

        tryCompare(lifecycleProbe, "departures", 1);
        compare(field.visible, false);
        compare(field.revealed, false);
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
        verify(field.revealed);
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

    // The newest notification is born at the column's origin — the seam
    // under the bell on the top routes, the screen's edge on the bottom one —
    // pushing the pile it joins away from that origin. It used to be born at
    // the far end, falling out of the previous toast, with the survivors
    // sliding back over an expired card's place; the author rejected both
    // movements on video.
    function test_the_newest_section_is_born_at_the_origin_edge_data() {
        return routeRows();
    }

    function test_the_newest_section_is_born_at_the_origin_edge(data) {
        const older = testCase.toast(70, "Old", 0);
        const window = createLifecycleStack(data.route, [older]);
        const field = fieldFor(window);
        settleLifecycleStack(window, field);

        window.toasts = [older, testCase.toast(71, "New", 0)];
        wait(CelestinaTheme.motionNormal * 2 + 40);

        const oldCard = findChild(window.contentItem,
                                  "celestina-toast-card-70");
        const newCard = findChild(window.contentItem,
                                  "celestina-toast-card-71");
        verify(oldCard !== null);
        verify(newCard !== null);
        verify(newCard.height > 0);
        if (data.route === "bottom")
            verify(newCard.y > oldCard.y);
        else
            verify(newCard.y < oldCard.y);
    }

    // SIMPLE-1: a finished block hands back its reveal, and the next burst
    // earns a fresh fade on the same persistent carrier.
    function test_a_finished_bottom_block_gets_a_fresh_reveal() {
        const window = createLifecycleStack(
            "bottom", [testCase.toast(50, "Old", 0)]);
        const field = fieldFor(window);
        settleLifecycleStack(window, field);

        window.toasts = [];
        tryCompare(lifecycleProbe, "departures", 1);
        compare(field.revealed, false);
        compare(field.presentationOpacity, 0);

        lifecycleProbe.departures = 0;
        window.toasts = [testCase.toast(51, "Fresh", 0)];
        tryVerify(function() {
            return field.revealed && field.presentationOpacity === 1;
        });
        compare(lifecycleProbe.departures, 0);
    }
}
