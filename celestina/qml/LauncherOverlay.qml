// The desktop-entry launcher: `Mod+Space`, a search field and a keyboard-driven
// list of results, in its own compositor surface. A keybind or command keeps it
// centred; the permanent panel button gives the same OverlayController a real
// opener rectangle. It answers the keyboard for its complete open lifetime.
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
import "ProviderReading.js" as ProviderReading

Window {
    id: overlay

    required property var providerSource
    required property bool reducedMotion
    property alias anchoredFromPanel: placement.anchoredFromPanel
    property alias openerRect: placement.openerRect
    property alias compositorBlurAvailable: card.compositorBlurAvailable
    property alias glassRects: card.glassRects
    property alias glassRegions: card.glassRegions

    signal dismissed()

    BackdropInk {
        id: backdropInk
    }

    readonly property int cardWidth: 620
    readonly property int cardHeight: 440
    readonly property int anchorGap: placement.anchorGap
    readonly property real cardX: placement.x
    readonly property real cardY: placement.y

    readonly property var launcherState: ProviderReading.read(overlay.providerSource, "launcher")
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

    onVisibleChanged: {
        if (visible)
            Qt.callLater(card.reveal);
    }

    PanelPopupPlacement {
        id: placement

        surfaceWidth: overlay.width
        surfaceHeight: overlay.height
        contentWidth: overlay.cardWidth
        contentHeight: overlay.cardHeight
        edgeInset: 0
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
                // Provider reasons are English diagnostics by contract. The
                // surface owns product copy and never paints those bytes.
                overlay.errorText = qsTr("No se pudo iniciar la aplicación");
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

    // A click anywhere outside the card closes this surface.
    //
    // The surface is the whole output, not the card: that is what makes an
    // outside click land here at all, and it is also what makes the panel
    // button that opened this close it in one click rather than two. While the
    // overlay is up the button is behind it, so the click never reaches the
    // panel, never re-enters `toggle()`, and focus returns exactly once.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        onPressed: overlay.dismissed()
    }

    Item {
        id: scene

        width: overlay.cardWidth
        height: overlay.cardHeight
        x: overlay.cardX
        y: overlay.cardY

        SoftOverlayCard {
            id: card
            ink: backdropInk
            anchors.fill: parent
            reducedMotion: overlay.reducedMotion
            accessibleName: qsTr("Buscador de aplicaciones")

            Item {
                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceMd

                MenuHeader {
                    id: launcherHeader

                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    ink: backdropInk
                    title: qsTr("Aplicaciones")
                    subtitle: {
                        if (!overlay.ready)
                            return qsTr("Preparando el índice");
                        if (overlay.truncated) {
                            return qsTr("%n resultado(s), lista parcial", "",
                                        overlay.rowCount);
                        }
                        return qsTr("%n resultado(s)", "", overlay.rowCount);
                    }
                    iconName: "app-window"
                }

                Item {
                    id: searchSection

                    anchors.top: launcherHeader.bottom
                    anchors.topMargin: CelestinaTheme.spaceSm
                    anchors.left: parent.left
                    anchors.right: parent.right
                    height: status.implicitHeight + CelestinaTheme.spaceMd * 2

                    MenuSection { ink: backdropInk }

                    // Everything above the list lays itself out top-down and
                    // takes only the height it needs. The results section gets
                    // the remainder without coupling its size to optional
                    // status lines.
                    Column {
                        id: status

                        anchors.fill: parent
                        anchors.margins: CelestinaTheme.spaceMd
                        spacing: CelestinaTheme.spaceMd

                        BackdropTextField {
                            id: searchField

                            width: parent.width
                            ink: backdropInk
                            shape: CelestinaTextField.Search
                            color: backdropInk.primary
                            placeholderTextColor: backdropInk.muted
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
                            color: backdropInk.danger
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontCaption
                            wrapMode: Text.Wrap
                        }

                        Text {
                            width: parent.width
                            visible: overlay.ready && overlay.rowCount === 0
                            text: qsTr("Sin resultados")
                            color: backdropInk.muted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontBody
                        }

                        Text {
                            width: parent.width
                            visible: !overlay.ready
                            text: qsTr("Preparando el índice de aplicaciones…")
                            color: backdropInk.muted
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontBody
                        }
                    }
                }

                Item {
                    anchors.top: searchSection.bottom
                    anchors.topMargin: CelestinaTheme.spaceSm
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom

                    MenuSection { ink: backdropInk }

                    ListView {
                        id: resultList

                        anchors.fill: parent
                        anchors.margins: CelestinaTheme.spaceSm
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
                        Accessible.onPressAction: {
                            overlay.currentIndex = row.index;
                            overlay.activateCurrent();
                        }

                        Rectangle {
                            anchors.fill: parent
                            radius: CelestinaTheme.radiusSm
                            color: rowMouse.pressed
                                   ? backdropInk.selectedFill
                                   : row.current
                                   ? backdropInk.selectedRestFill
                                   : rowMouse.containsMouse
                                     ? backdropInk.hoverFill : CelestinaTheme.clear
                        }

                        CelestinaIcon {
                            id: resultIcon

                            anchors.left: parent.left
                            anchors.leftMargin: CelestinaTheme.spaceSm
                            anchors.verticalCenter: parent.verticalCenter
                            width: CelestinaTheme.iconSm
                            height: width
                            name: row.isWebSearch ? "search" : "app-window"
                            fallbackName: "app-window"
                            tintOverride: row.current ? backdropInk.accent
                                                      : backdropInk.muted
                            Accessible.ignored: true
                        }

                        CelestinaIcon {
                            id: resultAction

                            anchors.right: parent.right
                            anchors.rightMargin: CelestinaTheme.spaceSm
                            anchors.verticalCenter: parent.verticalCenter
                            width: CelestinaTheme.iconSm
                            height: width
                            name: "go-next"
                            fallbackName: "go-next"
                            tintOverride: backdropInk.faint
                            Accessible.ignored: true
                        }

                        Column {
                            anchors.left: resultIcon.right
                            anchors.leftMargin: CelestinaTheme.spaceSm
                            anchors.right: resultAction.left
                            anchors.rightMargin: CelestinaTheme.spaceSm
                            anchors.verticalCenter: parent.verticalCenter

                            Text {
                                width: parent.width
                                text: row.isWebSearch
                                      ? qsTr("Buscar «%1» en la Web").arg(overlay.queryText)
                                      : row.entry.name
                                // A `.desktop` file is written by whichever
                                // package installed it; its name is shown as
                                // characters, not interpreted.
                                textFormat: Text.PlainText
                                color: row.current ? backdropInk.accent
                                                   : backdropInk.primary
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontRowSecondary
                                elide: Text.ElideRight
                            }
                            Text {
                                width: parent.width
                                visible: text.length > 0
                                text: row.subtitle
                                textFormat: Text.PlainText
                                color: backdropInk.muted
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
}
