import QtQuick
import org.celestina.calcita 1.0

// Carries the activation adapter's requests: a raise brings `host` forward
// (shown, raised, focused); an open hands each key to `CalcitaController`,
// which opens a window per document or raises the one already holding it.
// `source` is the `CalcitaActivation`; `host` is the front window, or any
// object with the same three functions.
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

        function onOpenRequested(keys) {
            keys.forEach(key => CalcitaController.openPath(key, ""))
        }
    }
}
