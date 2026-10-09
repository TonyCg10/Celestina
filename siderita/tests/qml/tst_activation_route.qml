import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// An `Open` from another launch, queued through the adapter, reaches the
// window: a folder opens its own tab, a file opens a tab on its folder with
// the file to select, and both bring the window forward.
TestCase {
    id: testCase
    name: "ActivationRoute"

    QtObject {
        id: activationStub
        signal raiseRequested()
        signal openFolderRequested(string folder)
        signal revealRequested(string folder, string item)
    }

    QtObject {
        id: windowStub
        property var tabs: []
        property int raised: 0
        function openTab(key, foreground, reveal) {
            windowStub.tabs = windowStub.tabs.concat([[key, foreground, reveal]])
        }
        function show() {}
        function raise() { windowStub.raised += 1 }
        function requestActivate() {}
    }

    ActivationRoute {
        source: activationStub
        host: windowStub
    }

    function init() {
        windowStub.tabs = []
        windowStub.raised = 0
    }

    function test_a_folder_opens_its_own_tab() {
        activationStub.openFolderRequested("/srv/fotos")
        compare(windowStub.tabs, [["/srv/fotos", true, ""]])
        compare(windowStub.raised, 1)
    }

    function test_a_file_opens_its_folder_with_the_file_selected() {
        activationStub.revealRequested("/srv", "/srv/a%20b.txt")
        compare(windowStub.tabs, [["/srv", true, "/srv/a%20b.txt"]])
        compare(windowStub.raised, 1)
    }

    function test_a_bare_launch_only_raises() {
        activationStub.raiseRequested()
        compare(windowStub.tabs, [])
        compare(windowStub.raised, 1)
    }
}
