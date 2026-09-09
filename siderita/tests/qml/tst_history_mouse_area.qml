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

    // The real arrangement: the history area lies *above* the content, which
    // takes only the ordinary buttons. Both halves matter — Back must reach
    // the area, and an ordinary click must still reach the content under it.
    // Sinking the area below the content instead looks equivalent and is not:
    // in the real window it then received no Back press at all.
    Item {
        id: layeredScene
        x: 0
        y: 120
        width: 240
        height: 40
        z: 2

        MouseArea {
            id: layeredContent
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
            hoverEnabled: true
            cursorShape: Qt.IBeamCursor
            onClicked: testCase.layeredContentClicks++
        }

        HistoryMouseArea {
            id: onTop
            anchors.fill: parent
            z: 1000
            canGoBack: true
            canGoForward: true
            onBackRequested: testCase.layeredBacks++
        }
    }
    property int layeredBacks: 0
    property int layeredContentClicks: 0

    function test_back_reaches_an_area_above_the_content() {
        layeredBacks = 0
        mouseClick(layeredScene, 120, 20, Qt.BackButton)
        compare(layeredBacks, 1)
        compare(backRequests, 0, "the press leaked past the scene")
    }

    function test_an_ordinary_click_still_reaches_the_content_below_it() {
        layeredContentClicks = 0
        mouseClick(layeredScene, 120, 20, Qt.LeftButton)
        compare(layeredContentClicks, 1)
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
