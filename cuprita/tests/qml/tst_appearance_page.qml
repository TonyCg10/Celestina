import QtQuick
import QtTest 1.3
import org.celestina.cuprita 1.0
import "../../qml/pages"
import "fakes"

// The Appearance page over the stand-in: the switch and the four sizes, a
// click and the arrows choose a size, Space toggles reduced motion, the
// environment's override disables the switch with its hint, and the loading
// row shows until the first reading.
TestCase {
    id: testCase
    name: "AppearancePage"
    width: 640
    height: 480
    visible: true
    when: windowShown

    Component {
        id: pageComponent

        Item {
            property alias page: appearancePage
            property alias fake: controller
            width: 640
            height: 480

            FakeAppearanceController { id: controller }

            AppearancePage {
                id: appearancePage
                anchors.fill: parent
                controller: controller
            }
        }
    }

    function test_the_switch_and_four_sizes() {
        const host = createTemporaryObject(pageComponent, testCase)
        const row = findChild(host.page, "reducedMotionRow")
        verify(row.visible)
        const toggle = findChild(row, "settingSwitch")
        compare(toggle.Accessible.name, "Reducir el movimiento")
        compare(toggle.checked, false)
        const choice = findChild(host.page, "textScaleChoice")
        compare(choice.model.length, 4)
        compare(choice.model.map(function(s) { return s.label }),
                ["Compacto", "Normal", "Grande", "Muy grande"])
        compare(choice.currentIndex, 1)
    }

    function test_choosing_large_sets_large() {
        const host = createTemporaryObject(pageComponent, testCase)
        const choice = findChild(host.page, "textScaleChoice")
        waitForRendering(host.page)
        mouseClick(findChild(choice, "segment-2"))
        compare(host.fake.calls, ["setTextScale:large"])
        tryCompare(choice, "currentIndex", 2)
    }

    function test_the_arrows_walk_the_sizes() {
        const host = createTemporaryObject(pageComponent, testCase)
        const choice = findChild(host.page, "textScaleChoice")
        choice.forceActiveFocus()
        keyClick(Qt.Key_Right)
        keyClick(Qt.Key_Right)
        keyClick(Qt.Key_Left)
        compare(host.fake.calls, ["setTextScale:large", "setTextScale:larger",
                                  "setTextScale:large"])
        compare(choice.currentIndex, 2)
    }

    function test_space_toggles_reduced_motion() {
        const host = createTemporaryObject(pageComponent, testCase)
        const toggle = findChild(findChild(host.page, "reducedMotionRow"), "settingSwitch")
        toggle.forceActiveFocus()
        keyClick(Qt.Key_Space)
        compare(host.fake.calls, ["setReducedMotion:true"])
        tryCompare(toggle, "checked", true)
    }

    function test_the_environment_forces_reduced_motion() {
        const host = createTemporaryObject(pageComponent, testCase)
        host.fake.forcedByEnvironment = true
        const row = findChild(host.page, "reducedMotionRow")
        const toggle = findChild(row, "settingSwitch")
        compare(toggle.checked, true)
        compare(row.enabled, false)
        const hint = findChild(row, "settingHint")
        verify(hint.visible)
        compare(hint.text, "Forzado por el entorno")
    }

    function test_loading_until_the_first_reading() {
        const host = createTemporaryObject(pageComponent, testCase)
        host.fake.loaded = false
        verify(findChild(host.page, "appearanceLoading").visible)
        verify(!findChild(host.page, "reducedMotionRow").visible)
        host.fake.loaded = true
        verify(!findChild(host.page, "appearanceLoading").visible)
        verify(findChild(host.page, "textScaleRow").visible)
    }

    function test_home_end_and_the_wrap() {
        const host = createTemporaryObject(pageComponent, testCase)
        const choice = findChild(host.page, "textScaleChoice")
        choice.forceActiveFocus()
        keyClick(Qt.Key_End)
        compare(choice.currentIndex, 3)
        keyClick(Qt.Key_Right)
        compare(choice.currentIndex, 0)
        keyClick(Qt.Key_Left)
        compare(choice.currentIndex, 3)
        keyClick(Qt.Key_Home)
        compare(choice.currentIndex, 0)
        compare(host.fake.calls, ["setTextScale:larger", "setTextScale:compact",
                                  "setTextScale:larger", "setTextScale:compact"])
    }

    function test_the_current_size_holds_the_focus() {
        const host = createTemporaryObject(pageComponent, testCase)
        const choice = findChild(host.page, "textScaleChoice")
        choice.forceActiveFocus()
        verify(findChild(choice, "segment-1").activeFocus)
        keyClick(Qt.Key_Right)
        tryVerify(function() { return findChild(choice, "segment-2").activeFocus })
    }

    function test_a_failed_save_keeps_the_switch_and_says_so() {
        const host = createTemporaryObject(pageComponent, testCase)
        host.fake.failing = true
        const spy = createTemporaryObject(signalSpyComponent, testCase,
                                          { target: host.fake, signalName: "notice" })
        const toggle = findChild(findChild(host.page, "reducedMotionRow"), "settingSwitch")
        toggle.forceActiveFocus()
        keyClick(Qt.Key_Space)
        compare(host.fake.calls, ["setReducedMotion:true"])
        compare(toggle.checked, false)
        compare(spy.count, 1)
        compare(spy.signalArguments[0][0], "error")
    }

    function test_the_choice_stays_inside_a_narrow_card() {
        const host = createTemporaryObject(pageComponent, testCase)
        host.width = 300
        host.fake.textScale = "larger"
        const choice = findChild(host.page, "textScaleChoice")
        const row = findChild(host.page, "textScaleRow")
        tryVerify(function() { return choice.x + choice.width <= row.width })
        verify(choice.width < choice.implicitWidth)
    }

    Component {
        id: signalSpyComponent
        SignalSpy {}
    }
}
