import QtQuick

// Carries the activation adapter's requests to the window: when another launch
// reaches this process, the window is shown, raised and given the focus.
// `source` is the `SelenitaActivation`; `host` is the window, or any object
// with the same three functions.
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
    }
}
