import QtQuick
import QtTest
import "../qml" as Desktop

// The panel presence is deliberately one compact pile rather than an app
// dock: it appears only for compositor-minimized windows, caps the painted
// overlap, and routes every activation to the selector.
TestCase {
    id: testCase

    name: "BubbleGroup"
    when: windowShown
    visible: true
    width: 180
    height: 80

    Desktop.BackdropInk { id: testInk }

    QtObject {
        id: reading

        property bool available: true
        property var windows: []
    }

    Desktop.BubbleGroup {
        id: group

        anchors.centerIn: parent
        reading: reading
        ink: testInk
    }

    SignalSpy {
        id: selectorRequests

        target: group
        signalName: "selectorRequested"
    }

    function windows(count) {
        const result = [];
        for (let index = 0; index < count; ++index) {
            result.push({
                "id": String(index + 1),
                "appId": "org.example.App" + index,
                "title": "Ventana " + index
            });
        }
        return result;
    }

    function init() {
        selectorRequests.clear();
        reading.available = true;
        reading.windows = [];
    }

    function test_empty_or_unavailable_state_paints_no_launcher() {
        compare(group.bubbleCount, 0);
        verify(!group.visible);

        reading.windows = windows(2);
        reading.available = false;
        compare(group.bubbleCount, 0);
        verify(!group.visible);
    }

    function test_the_stack_caps_overlap_and_reports_the_remainder() {
        reading.windows = windows(5);

        compare(group.bubbleCount, 5);
        compare(group.visibleBubbleCount, 3);
        compare(group.implicitWidth,
                group.bubbleSize + 2 * group.overlapStep);
        const overflow = findChild(group, "celestina-bubble-overflow");
        verify(overflow !== null);
        verify(overflow.visible);
    }

    function test_pointer_and_keyboard_share_the_selector_route() {
        reading.windows = windows(2);
        mouseClick(group, group.width / 2, group.height / 2,
                   Qt.LeftButton);
        compare(selectorRequests.count, 1);

        group.forceActiveFocus(Qt.TabFocusReason);
        keyClick(Qt.Key_Return);
        compare(selectorRequests.count, 2);
    }
}
