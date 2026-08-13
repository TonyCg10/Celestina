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

    // The stack is one surface carrying several cards of the shell's glass,
    // so it publishes the union of their regions. A toast used to be a
    // `GlassCard` capturing a scene that holds nothing behind it, which fell
    // back to an opaque tint and published no region at all.
    function test_every_toast_is_glass_and_the_stack_publishes_all_of_it() {
        stack.show();
        tryCompare(stack, "visible", true);
        tryVerify(function() {
            return stack.glassRegions.length === stack.toasts.length;
        });
        compare(stack.glassRects.length, stack.toasts.length);

        const body = findChild(stack.contentItem, "celestina-menu-body-tint");
        verify(body);
        compare(body.backdropMode, GlassSurface.ExternalBackdrop);
        compare(body.captureActive, false);
        compare(body.materialRole, GlassSurface.ContextualVeil);
        compare(body.elevation, 0);

        const section = findChild(stack.contentItem, "celestina-menu-section");
        verify(section);
        compare(section.materialRole, GlassSurface.ContentSurface);
        verify(body.materialStrength < section.materialStrength);
        stack.hide();
    }

    // Attached, only the first card grips the bar — the membrane is one drop
    // out of the bell — and the rest of the column hangs from it, each card
    // still knowing where it sits on the output.
    function test_only_the_first_toast_grips_the_bar() {
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
        compare(fields.length, stack.toasts.length);
        // The column keeps the model's order top to bottom, so the gripping
        // card is the one whose y is the column's own top.
        fields.sort(function(a, b) { return a.y - b.y; });
        verify(fields[0].attachedToTop);
        verify(!fields[1].attachedToTop);
        verify(fields[0].surfacePosition.y < fields[1].surfacePosition.y);

        stack.anchoredFromPanel = false;
        stack.attachmentStartY = -1;
        stack.surfaceOriginX = 0;
        stack.surfaceWidth = stack.cardWidth;
        stack.surfaceHeight = 0;
    }
}
