import QtQuick

// Window-level owner for the two history buttons. It deliberately accepts
// only Back/Forward, so ordinary clicks keep reaching the controls below it.
MouseArea {
    id: root

    required property bool canGoBack
    required property bool canGoForward
    property bool blocked: false

    signal backRequested
    signal forwardRequested

    enabled: !blocked
    acceptedButtons: Qt.BackButton | Qt.ForwardButton
    preventStealing: true

    onPressed: function(mouse) {
        if (mouse.button === Qt.BackButton && root.canGoBack)
            root.backRequested()
        else if (mouse.button === Qt.ForwardButton && root.canGoForward)
            root.forwardRequested()
    }
}
