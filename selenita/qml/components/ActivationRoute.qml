import QtQuick
import org.celestina.selenita 1.0

// Carries the activation adapter's requests to the window: when another launch
// reaches this process, the window is shown, raised and given the focus; when
// a key binding asks for a capture, the target is chosen and the capture
// taken. `source` is the `SelenitaActivation`; `host` is the window, or any
// object with the same four functions.
QtObject {
    id: route
    required property QtObject source
    required property var host

    readonly property Connections connections: Connections {
        target: route.source

        function onRaiseRequested() {
            route.host.show()
            route.host.raise()
            route.host.requestActivate()
        }

        function onCaptureRequested(target: string) {
            SelenitaController.target = target
            route.host.startCapture()
        }
    }
}
