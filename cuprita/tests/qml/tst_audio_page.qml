import QtQuick
import QtTest 1.3
import org.celestina.cuprita 1.0
import "../../qml/pages"
import "fakes"

// The Audio page over the scripted graph: the output selector lists both
// sinks and choosing the second makes it the default; the mute icon toggles.
TestCase {
    id: testCase
    name: "AudioPage"
    width: 640
    height: 600
    visible: true
    when: windowShown

    Component {
        id: pageComponent

        Item {
            id: host
            property alias page: audioPage
            property alias fake: controller
            width: 640
            height: 600

            FakeAudioController { id: controller }

            AudioPage {
                id: audioPage
                anchors.fill: parent
                controller: controller
                sinks: controller.sinks
                sources: controller.sources
                streams: controller.streams
                backdropSource: host
            }
        }
    }

    function test_selector_lists_two_sinks_and_sets_the_default() {
        const host = createTemporaryObject(pageComponent, testCase)
        const card = findChild(host.page, "outputCard")
        const selector = findChild(card, "endpointSelector")
        compare(selector.text, "Speakers")
        waitForRendering(host.page)
        waitForRendering(host.page)
        mouseClick(selector)
        const menu = findChild(card, "endpointMenu")
        tryCompare(menu, "opened", true)
        compare(menu.count, 2)
        const second = menu.itemAt(1)
        compare(second.text, "HDMI")
        waitForRendering(second)
        mouseClick(second)
        compare(host.fake.calls, ["setDefault:41"])
        tryCompare(selector, "text", "HDMI")
    }

    function test_mute_icon_toggles_muted() {
        const host = createTemporaryObject(pageComponent, testCase)
        const card = findChild(host.page, "outputCard")
        const mute = findChild(card, "muteButton")
        compare(mute.iconName, "media-volume")
        waitForRendering(host.page)
        waitForRendering(host.page)
        mouseClick(mute)
        compare(host.fake.calls, ["setMuted:40:true"])
        tryCompare(mute, "iconName", "media-volume-muted")
        mouseClick(mute)
        compare(host.fake.calls, ["setMuted:40:true", "setMuted:40:false"])
    }

    function test_input_mute_toggles_the_source() {
        const host = createTemporaryObject(pageComponent, testCase)
        const card = findChild(host.page, "inputCard")
        const mute = findChild(card, "muteButton")
        compare(mute.iconName, "mic")
        waitForRendering(host.page)
        mouseClick(mute)
        compare(host.fake.calls, ["setMuted:50:true"])
        tryCompare(mute, "iconName", "mic-off")
    }
}
