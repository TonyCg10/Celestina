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
        // One frame is enough: a second wait finds the scene idle and only
        // times out.
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

    function profileCards(page) {
        const found = []
        const stack = [page]
        while (stack.length > 0) {
            const item = stack.pop()
            if (item.objectName === "profileCard" && item.visible)
                found.push(item)
            for (let i = 0; i < item.children.length; ++i)
                stack.push(item.children[i])
        }
        return found
    }

    function test_profile_cards_follow_the_cards_with_a_choice() {
        const host = createTemporaryObject(pageComponent, testCase)
        // The scripted card has two profiles: one card, showing the active.
        compare(profileCards(host.page).length, 1)
        compare(findChild(host.page, "profileSelector").text, "Analog Stereo")
        // A second sound card with a choice of its own gets its own card.
        host.fake.profiles = host.fake.profiles.concat([
            { cardId: 31, id: "output:hdmi-stereo", description: "HDMI 1", active: true },
            { cardId: 31, id: "output:hdmi-stereo-extra1", description: "HDMI 2", active: false }
        ])
        tryVerify(() => profileCards(host.page).length === 2)
        // No card with more than one profile: no profile card at all.
        host.fake.profiles = []
        compare(profileCards(host.page).length, 0)
    }

    function test_the_volume_slider_runs_to_150_with_a_tick_at_100() {
        const host = createTemporaryObject(pageComponent, testCase)
        const card = findChild(host.page, "outputCard")
        const tick = findChild(card, "fullVolumeTick")
        verify(tick !== null)
        compare(tick.parent.to, 150)
        compare(tick.parent.step, 1)
    }

    function test_a_dragged_slider_writes_at_most_every_100_ms() {
        const host = createTemporaryObject(pageComponent, testCase)
        const card = findChild(host.page, "outputCard")
        const slider = findChild(card, "fullVolumeTick").parent
        slider.moved(70)
        slider.moved(80)
        slider.moved(90)
        // The first value goes at once; the rest wait for the throttle.
        compare(host.fake.calls, ["setVolume:40:0.7"])
        tryCompare(host.fake, "calls", ["setVolume:40:0.7", "setVolume:40:0.9"])
        // A drag's end sends its last value without waiting.
        slider.moved(100)
        slider.moved(110)
        slider.dragging = true
        slider.dragging = false
        compare(host.fake.calls[host.fake.calls.length - 1], "setVolume:40:1.1")
    }

    function test_the_audio_cards_show_a_loading_row_until_the_first_snapshot() {
        const host = createTemporaryObject(pageComponent, testCase)
        host.fake.loaded = false
        const streams = findChild(host.page, "streamsLoading")
        const output = findChild(findChild(host.page, "outputCard"), "endpointLoading")
        tryCompare(streams, "visible", true)
        tryCompare(output, "visible", true)
        verify(!findChild(findChild(host.page, "outputCard"), "endpointSelector").visible)
        host.fake.snapshot()
        tryCompare(streams, "visible", false)
        tryCompare(output, "visible", false)
    }
}
