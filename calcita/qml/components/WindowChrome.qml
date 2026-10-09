import QtQuick
import org.celestina.calcita 1.0

// What both of Calcita's windows carry over their content: the drop area for
// `text/uri-list`, the notice pill and Ctrl+O. `origin` names the window to
// the controller (a document's key, or `main`): a refusal of what this
// window was given comes back here, whether or not the window is active.
// `fallback` makes this window also show the notices no window asked for.
Item {
    id: chrome

    required property string origin
    property bool fallback: false

    // Ctrl+O: the owner shows the file chooser.
    signal openRequested()

    function showNotice(kind, text) {
        noticePill.show(kind, text)
    }

    anchors.fill: parent

    DropArea {
        objectName: "dropArea"
        anchors.fill: parent
        keys: ["text/uri-list"]
        onDropped: drop => {
            CalcitaController.openDropped(drop.urls.map(url => url.toString()), chrome.origin)
            drop.accept()
        }
    }

    NoticePill {
        id: noticePill
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceXl
        anchors.horizontalCenter: parent.horizontalCenter
    }

    Connections {
        target: CalcitaController
        function onNotice(kind, text, origin) {
            if (origin === chrome.origin || (origin === "" && chrome.fallback))
                noticePill.show(kind, text)
        }
    }

    Shortcut {
        sequences: [StandardKey.Open]
        onActivated: chrome.openRequested()
    }
}
