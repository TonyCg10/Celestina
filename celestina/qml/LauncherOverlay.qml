// The desktop-entry launcher: `Mod+Space`, a search field and a keyboard-driven
// list of results, in its own compositor surface — the same
// `OverlayController`/`OverlaySurface` mechanics as the panel's menu, but
// centered rather than anchored, and answering the keyboard the whole time it
// is open rather than only until a click.
//
// Every provider verb this component sends — `query`, `launch`, `web-search`
// — goes straight to `providerSource`, exactly the way every bar widget
// already talks to it (see `Panel.qml`); nothing here is routed back through a
// controller. Results carry no application icon: the launcher provider
// publishes identifiers and text, not pixmaps, and resolving a `.desktop`
// entry's `Icon=` through the freedesktop icon theme is a separate, unbuilt
// feature this phase does not need to gate on.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: overlay

    required property var providerSource
    required property bool reducedMotion

    signal dismissed()

    readonly property int cardWidth: 620
    readonly property int cardHeight: 440

    readonly property var launcherState: providerSource && providerSource.providers
                                          ? providerSource.providers.launcher : undefined
    readonly property bool ready: launcherState !== undefined
                                  && launcherState.ready === true
    readonly property var hits: launcherState && launcherState.hits !== undefined
                                ? launcherState.hits : []
    readonly property bool truncated: launcherState !== undefined
                                      && launcherState.truncated === true
    readonly property string queryText: searchField.text
    // One more row than the provider published: activating it searches the
    // Web instead of launching an entry. Kept in the same list and the same
    // selection model as every other row, so Up/Down/Enter never special-case
    // it.
    readonly property bool offersWebSearch: queryText.length > 0
    readonly property int rowCount: hits.length + (offersWebSearch ? 1 : 0)

    property int currentIndex: -1
    property string errorText: ""
    property int pendingLaunchRequest: -1

    width: cardWidth
    height: cardHeight
    color: CelestinaTheme.clear
    title: qsTr("Buscador de aplicaciones")

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = reducedMotion;
        searchField.forceActiveFocus();
    }

    // Not `onHitsChanged`: `hits` is a `var` sliced out of `providerSource`'s
    // aggregate `providers` map, which changes reference every time *any* bar
    // provider republishes — CPU, audio, whatever's next — even when the
    // launcher's own results are byte-for-byte identical. Resetting on that
    // reset the highlighted row back to the top a few seconds into every
    // arrow-key session. `queryText` and `rowCount` are a plain string and an
    // int, and QML only signals their `Changed` when the value itself differs.
    onQueryTextChanged: overlay.currentIndex = overlay.rowCount > 0 ? 0 : -1
    onRowCountChanged: {
        if (overlay.currentIndex >= overlay.rowCount)
            overlay.currentIndex = overlay.rowCount > 0 ? overlay.rowCount - 1 : -1;
        else if (overlay.currentIndex < 0 && overlay.rowCount > 0)
            overlay.currentIndex = 0;
    }

    Connections {
        target: overlay.providerSource
        function onCommandResult(requestId, state, reason) {
            if (requestId !== overlay.pendingLaunchRequest)
                return;
            overlay.pendingLaunchRequest = -1;
            if (state === "accepted") {
                overlay.dismissed();
            } else {
                overlay.errorText = reason.length > 0
                        ? reason : qsTr("No se pudo iniciar la aplicación");
            }
        }
    }

    function sendQuery(text) {
        if (overlay.providerSource)
            overlay.providerSource.sendCommand("launcher", "query", {"query": text});
    }

    function activateCurrent() {
        if (overlay.currentIndex < 0 || overlay.currentIndex >= overlay.rowCount)
            return;
        if (overlay.currentIndex < overlay.hits.length) {
            const entry = overlay.hits[overlay.currentIndex];
            if (!overlay.providerSource)
                return;
            overlay.errorText = "";
            overlay.pendingLaunchRequest = overlay.providerSource.sendCommand(
                        "launcher", "launch", {"id": entry.id});
        } else if (overlay.providerSource) {
            overlay.providerSource.sendCommand(
                        "launcher", "web-search", {"query": overlay.queryText});
            overlay.dismissed();
        }
    }

    Item {
        id: scene
        anchors.fill: parent

        GlassCard {
            id: card
            anchors.fill: parent
            backdropSource: scene
            Accessible.role: Accessible.Dialog
            Accessible.name: qsTr("Buscador de aplicaciones")

            Item {
                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceLg

                // Everything above the list lays itself out top-down and only
                // takes the height it needs; the list gets the rest. Doing
                // this by anchors rather than one `Column` including the list
                // means the list's height never has to be computed by hand
                // from its siblings — two of which (the error line, the
                // "still indexing" line) can be visible at once while typing
                // during the narrow startup window.
                Column {
                    id: status

                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    spacing: CelestinaTheme.spaceMd

                    CelestinaTextField {
                        id: searchField

                        width: parent.width
                        shape: CelestinaTextField.Search
                        placeholderText: qsTr("Buscar aplicaciones…")
                        Accessible.name: qsTr("Buscar aplicaciones")
                        onTextChanged: overlay.sendQuery(text)

                        Keys.onPressed: function(event) {
                            if (event.key === Qt.Key_Escape) {
                                overlay.dismissed();
                            } else if (event.key === Qt.Key_Down) {
                                if (overlay.rowCount > 0)
                                    overlay.currentIndex = Math.min(
                                            overlay.rowCount - 1, overlay.currentIndex + 1);
                            } else if (event.key === Qt.Key_Up) {
                                if (overlay.rowCount > 0)
                                    overlay.currentIndex = Math.max(0, overlay.currentIndex - 1);
                            } else if (event.key === Qt.Key_Return
                                       || event.key === Qt.Key_Enter) {
                                overlay.activateCurrent();
                            } else {
                                return;
                            }
                            event.accepted = true;
                        }
                    }

                    Text {
                        width: parent.width
                        visible: overlay.errorText.length > 0
                        text: overlay.errorText
                        color: CelestinaTheme.dangerFillInk
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                        wrapMode: Text.Wrap
                    }

                    Text {
                        width: parent.width
                        visible: overlay.ready && overlay.rowCount === 0
                        text: qsTr("Sin resultados")
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontBody
                    }

                    Text {
                        width: parent.width
                        visible: !overlay.ready
                        text: qsTr("Preparando el índice de aplicaciones…")
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontBody
                    }
                }

                ListView {
                    id: resultList

                    anchors.top: status.bottom
                    anchors.topMargin: CelestinaTheme.spaceMd
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    clip: true
                    spacing: 2
                    visible: overlay.rowCount > 0
                    model: overlay.rowCount
                    currentIndex: overlay.currentIndex
                    onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
                    Accessible.role: Accessible.List
                    Accessible.name: qsTr("Resultados de la búsqueda")

                    delegate: Item {
                        id: row

                        required property int index
                        readonly property bool isWebSearch: index >= overlay.hits.length
                        readonly property var entry: isWebSearch ? null : overlay.hits[index]
                        readonly property bool current: overlay.currentIndex === row.index

                        readonly property string subtitle: isWebSearch ? ""
                                : (entry.genericName || entry.comment || "")

                        width: ListView.view.width
                        height: row.subtitle.length > 0 ? 46 : 34
                        Accessible.role: Accessible.ListItem
                        Accessible.name: isWebSearch
                                ? qsTr("Buscar «%1» en la Web").arg(overlay.queryText)
                                : entry.name
                        Accessible.selected: row.current

                        Rectangle {
                            anchors.fill: parent
                            radius: CelestinaTheme.radiusSm
                            color: row.current
                                   ? CelestinaTheme.badgeAccentFill
                                   : rowMouse.containsMouse
                                     ? CelestinaTheme.surfaceHover : CelestinaTheme.clear
                        }

                        Column {
                            x: CelestinaTheme.spaceSm
                            anchors.verticalCenter: parent.verticalCenter
                            width: parent.width - CelestinaTheme.spaceSm * 2

                            Text {
                                width: parent.width
                                text: row.isWebSearch
                                      ? qsTr("Buscar «%1» en la Web").arg(overlay.queryText)
                                      : row.entry.name
                                color: row.current ? CelestinaTheme.accent : CelestinaTheme.text
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontRowSecondary
                                elide: Text.ElideRight
                            }
                            Text {
                                width: parent.width
                                visible: text.length > 0
                                text: row.subtitle
                                color: CelestinaTheme.textMuted
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontMini
                                elide: Text.ElideRight
                            }
                        }

                        MouseArea {
                            id: rowMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: {
                                overlay.currentIndex = row.index;
                                overlay.activateCurrent();
                            }
                        }
                    }
                }
            }
        }
    }
}
