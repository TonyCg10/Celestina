import QtQuick

// Routes the activation adapter's queued requests to the window: a folder
// handed by another launch opens in a tab of its own, a file opens a tab on
// its folder with the file selected, and either brings the window forward.
// `source` is the `SideritaActivation`; `host` is the window, or a stand-in
// under test, offering `openTab(key, foreground, reveal)`.
QtObject {
    id: route
    required property QtObject source
    required property var host

    function bringForward() {
        route.host.show()
        route.host.raise()
        route.host.requestActivate()
    }

    readonly property Connections connections: Connections {
        target: route.source

        function onRaiseRequested() {
            route.bringForward()
        }

        function onOpenFolderRequested(folder) {
            route.host.openTab(folder, true, "")
            route.bringForward()
        }

        function onRevealRequested(folder, item) {
            route.host.openTab(folder, true, item)
            route.bringForward()
        }
    }
}
