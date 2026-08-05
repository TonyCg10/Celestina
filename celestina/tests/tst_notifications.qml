import QtQuick
import QtTest
import "../qml" as Desktop

// What the panel's indicator and the notification centre make of what the
// server published. Constructed offscreen: this proves the components build
// and read truthfully, never how they look on a compositor.
TestCase {
    id: testCase

    name: "Notifications"

    function entry(id, app, summary, actions) {
        return {
            "id": id,
            "app": app,
            "summary": summary,
            "body": "",
            "urgency": "normal",
            "read": false,
            "actions": actions === undefined ? [] : actions
        };
    }

    Desktop.NotificationIndicator {
        id: indicator

        reading: undefined
    }

    Desktop.NotificationCenter {
        id: centre

        providerSource: null
        reducedMotion: false
    }

    SignalSpy {
        id: dismissedSpy

        target: centre
        signalName: "dismissed"
    }

    function test_escape_dismisses_the_centre_at_the_window_boundary() {
        dismissedSpy.clear();
        centre.show();
        centre.requestActivate();
        tryCompare(centre, "active", true);

        keyClick(Qt.Key_Escape);
        compare(dismissedSpy.count, 1);
        centre.hide();
    }

    function test_nothing_to_say_is_nothing_to_show() {
        indicator.reading = {"unread": 0, "quiet": false};
        verify(indicator.serving);
        verify(!indicator.worthShowing);
        compare(indicator.implicitWidth, 0);
    }

    function test_a_count_appears_only_when_there_is_one() {
        indicator.reading = {"unread": 3, "quiet": false};
        verify(indicator.worthShowing);
        compare(indicator.unread, 3);
    }

    function test_being_silenced_is_always_worth_showing() {
        // Even with nothing waiting: a person who silenced their session needs
        // to be able to tell that they did.
        indicator.reading = {"unread": 0, "quiet": true};
        verify(indicator.quiet);
        verify(indicator.worthShowing);
    }

    function test_no_server_is_not_the_same_as_nothing_waiting() {
        indicator.reading = undefined;
        verify(!indicator.serving);
        verify(!indicator.worthShowing);

        // The centre says so in words rather than showing an empty list.
        verify(!centre.serving);
        compare(centre.entries.length, 0);
    }

    function test_the_centre_lists_what_is_live_before_what_ended() {
        centre.providerSource = {
            "providers": {
                "notifications": {
                    "unread": 2,
                    "quiet": false,
                    "historyTruncated": false,
                    "toasts": [testCase.entry(7, "Magnetita", "live")],
                    "history": [testCase.entry(4, "Magnetita", "ended")]
                }
            }
        };

        verify(centre.serving);
        compare(centre.live.length, 1);
        compare(centre.past.length, 1);
        compare(centre.entries.length, 2);
        compare(centre.entries[0].summary, "live");
        compare(centre.entries[1].summary, "ended");
    }

    function test_only_a_live_notification_can_still_be_acted_on() {
        centre.providerSource = {
            "providers": {
                "notifications": {
                    "unread": 1,
                    "quiet": false,
                    "historyTruncated": false,
                    "toasts": [testCase.entry(7, "Magnetita", "live",
                                              [{"key": "open", "label": "Open"}])],
                    "history": [testCase.entry(4, "Magnetita", "ended",
                                               [{"key": "open", "label": "Open"}])]
                }
            }
        };

        // Index 1 is the ended one: a producer that already withdrew its
        // notification is not waiting for an answer, so nothing is sent.
        centre.invokeFirst(1);
        centre.dismiss(1);
        // Nothing to assert but the absence of a crash and of a command: the
        // fake provider source above has no sendCommand at all, so any attempt
        // to send one would fail this case.
        verify(true);
    }
}
