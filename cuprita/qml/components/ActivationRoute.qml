import QtQuick

// Routes the activation adapter's queued requests to the window: another
// launch (with or without paths) brings this one to the front. `source` is
// the `CupritaActivation`; `host` is the window, or a stand-in under test.
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
