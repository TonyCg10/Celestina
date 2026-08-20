import QtQuick
import org.celestina.siderita 1.0

// The phone's media action, in whichever bar is carrying the location.
//
// While the heading is on screen it rides inside it; once the heading retires,
// the copy in the folder view hangs under the search glyph — anchored, so it
// follows the bar. Positioning it with `mapToItem` instead put it in the corner
// and left it there: that call is evaluated once and never re-evaluated.
//
// It used to live inside the heading, which is where it belongs while the
// heading is on screen — but the heading now retires on a scroll, and an action
// must not leave with the furniture it was standing on. Two placements, one
// component, so the icon, the wording and the disabled state cannot drift apart.
CelestinaIconButton {
    id: root

    required property bool connected

    density: CelestinaButton.Regular
    iconName: "music"
    fallbackIcon: "audio-x-generic"
    enabled: root.connected
    Accessible.name: root.connected ? qsTr("Multimedia del móvil")
                                    : qsTr("Móvil desconectado")
}
