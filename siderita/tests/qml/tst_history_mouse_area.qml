import QtQuick 2.15
import QtTest 1.3
import "../../qml/components/chrome"

TestCase {
    id: testCase
    name: "HistoryMouseArea"
    width: 240
    height: 160
    visible: true
    when: windowShown

    property int backRequests: 0
    property int forwardRequests: 0
    property int leftClicks: 0

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onClicked: function(mouse) {
            if (mouse.button === Qt.LeftButton)
                testCase.leftClicks++
        }
    }

    HistoryMouseArea {
        id: historyButtons
        anchors.fill: parent
        z: 1
        canGoBack: true
        canGoForward: true
        onBackRequested: testCase.backRequests++
        onForwardRequested: testCase.forwardRequests++
    }

    function init() {
        backRequests = 0
        forwardRequests = 0
        leftClicks = 0
        historyButtons.blocked = false
        historyButtons.canGoBack = true
        historyButtons.canGoForward = true
    }

    function test_routes_history_buttons_only() {
        mouseClick(testCase, 80, 80, Qt.BackButton)
        mouseClick(testCase, 80, 80, Qt.ForwardButton)
        compare(backRequests, 1)
        compare(forwardRequests, 1)

        mouseClick(testCase, 80, 80, Qt.LeftButton)
        compare(leftClicks, 1)
    }

    function test_respects_state() {
        historyButtons.canGoBack = false
        historyButtons.canGoForward = false
        mouseClick(testCase, 80, 80, Qt.BackButton)
        mouseClick(testCase, 80, 80, Qt.ForwardButton)
        compare(backRequests, 0)
        compare(forwardRequests, 0)

        historyButtons.canGoBack = true
        historyButtons.blocked = true
        mouseClick(testCase, 80, 80, Qt.BackButton)
        compare(backRequests, 0)
    }
}
