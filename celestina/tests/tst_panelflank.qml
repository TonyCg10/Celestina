import QtQuick
import QtTest
import CelestinaStyle
import "../qml" as Desktop

// The live failure: the media widget never appeared on either panel. Not a data
// problem — the provider published a valid player — but a layout one. The
// workspace strip grows with the focused window's title and was allowed to
// claim the whole flank; the flank clips, so everything after the strip left
// the bar without a word.
TestCase {
    id: testCase

    name: "PanelFlank"

    readonly property string longTitle:
        "Firefox — lofi hip hop radio 📚 beats to relax/study to — a title long " +
        "enough to fill the whole flank and then some, which is what the live " +
        "session had when the media widget vanished"

    function workspaces(title) {
        return [{
            "index": 1, "label": "1", "output": "DP-1", "active": true,
            "focused": true, "urgent": false, "activeWindowTitle": title,
            "requestState": ""
        }];
    }

    Item {
        id: host

        width: 600
        height: 40

        Desktop.PanelFlank {
            id: flank

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            // The same expression Panel.qml uses, so this regression measures
            // the shipped rule rather than a copy of it.
            reservedWidth: flank.roomFor(sysMon) + flank.roomFor(media)

            Desktop.WorkspaceStrip {
                id: strip

                width: Math.min(implicitWidth,
                                Math.max(0, flank.width - flank.reservedWidth))
                niriAvailable: true
                outputName: "DP-1"
                workspaces: testCase.workspaces("")
            }

            Desktop.SysMon {
                id: sysMon

                reading: ({"cpu": 12, "memory": 34, "load": "calm"})
            }

            Desktop.MediaMini {
                id: media

                reading: ({
                    "nowPlaying": "Lofi Girl - lofi hip hop radio",
                    "playing": true,
                    "progress": "live"
                })
            }
        }
    }

    // `visible` also depends on the test's own window being shown, which an
    // offscreen run does not do; what this is about is whether the widget has
    // width to occupy in the row.
    function test_a_valid_player_has_width() {
        verify(media.hasPlayer);
        verify(media.implicitWidth > 0);
    }

    function test_a_long_window_title_never_pushes_the_media_widget_off_the_bar() {
        strip.workspaces = testCase.workspaces(testCase.longTitle);
        wait(0);

        // The strip wants more than the flank has...
        verify(strip.implicitWidth > flank.width,
               "the fixture must actually overflow the flank");
        // ...and is held to what its neighbours do not need.
        verify(strip.width <= flank.width - flank.reservedWidth + 1);

        // Which is the whole point: the media widget still fits inside the
        // flank rather than being clipped out of existence.
        verify(media.hasPlayer);
        verify(media.implicitWidth > 0);
        verify(strip.width + flank.reservedWidth <= flank.width + 1);
    }

    function test_an_absent_widget_reserves_nothing() {
        const withMedia = flank.reservedWidth;
        media.reading = undefined;
        wait(0);

        verify(!media.hasPlayer);
        verify(flank.reservedWidth < withMedia,
               "a widget that is not there must not hold space open");

        media.reading = ({"nowPlaying": "something", "playing": false, "progress": "live"});
        wait(0);
        verify(media.hasPlayer);
        verify(flank.reservedWidth > 0);
    }
}
