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
}
