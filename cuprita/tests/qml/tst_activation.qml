import QtQuick
import QtTest 1.3
import org.celestina.cuprita 1.0
import "../../qml/components"
import "fakes"

// An `Open` from another launch, queued through the adapter as a raise,
// reaches the window: shown, raised and activated, in that order.
TestCase {
    id: testCase
    name: "Activation"

    Component {
        id: hostComponent

        Item {
            id: stage
            property alias activation: fake
            property var calls: []
            function show() { stage.calls = stage.calls.concat(["show"]) }
            function raise() { stage.calls = stage.calls.concat(["raise"]) }
            function requestActivate() { stage.calls = stage.calls.concat(["activate"]) }

            FakeActivation { id: fake }

            ActivationRoute {
                source: fake
                host: stage
            }
        }
    }

    function test_a_queued_open_raises_the_window() {
        const host = createTemporaryObject(hostComponent, testCase)
        host.activation.raiseRequested()
        compare(host.calls, ["show", "raise", "activate"])
    }
}
