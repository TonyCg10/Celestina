// Everything the session has said lately, and what can still be done about it.
//
// This is the keyboard path. A toast never takes focus — interrupting typing is
// the one thing a notification must not do — so every action a toast offers is
// reachable here instead: Up/Down to choose, Enter for the first action,
// Delete to dismiss, and one key for do-not-disturb. That split is what lets
// the corner stay unfocused without putting any control out of reach.
//
// It shows what the server published: what is still live, then what has ended.
// Nothing here decides expiry, caps or ordering.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window
import "ProviderReading.js" as ProviderReading

Window {
    id: centre

    required property var providerSource
    required property bool reducedMotion

    signal dismissed()

    Shortcut {
        sequence: "Escape"
        context: Qt.WindowShortcut
        onActivated: centre.dismissed()
    }

    readonly property int cardWidth: 460
    readonly property int cardHeight: 520

    readonly property var state: ProviderReading.read(centre.providerSource, "notifications")
    // `undefined` means this shell is not the session's notification server —
    // another one owns the name — which is a different thing from having
    // nothing to show.
    readonly property bool serving: centre.state !== undefined
                                    && centre.state.unread !== undefined
    readonly property bool quiet: centre.serving && centre.state.quiet === true
    readonly property var live: centre.serving && centre.state.toasts !== undefined
                                ? centre.state.toasts : []
    readonly property var past: centre.serving && centre.state.history !== undefined
                                ? centre.state.history : []
    readonly property var entries: centre.live.concat(centre.past)
    readonly property bool truncated: centre.serving
                                      && centre.state.historyTruncated === true
    // Actions arrive beside the notifications, each naming the one it belongs
    // to: the host takes one level of structure, so a notification carrying its
    // own list of actions is a frame it refuses.
    readonly property var offeredActions: centre.serving && centre.state.actions !== undefined
                                          ? centre.state.actions : []

    function actionsFor(id) {
        const mine = [];
        for (let index = 0; index < centre.offeredActions.length; ++index) {
            if (centre.offeredActions[index].notification === id)
                mine.push(centre.offeredActions[index]);
        }
        return mine;
    }

    property int currentIndex: entries.length > 0 ? 0 : -1

    width: cardWidth
    height: cardHeight
    color: CelestinaTheme.clear
    title: qsTr("Notificaciones")

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = centre.reducedMotion;
        list.forceActiveFocus();
    }

    onEntriesChanged: {
        if (centre.currentIndex >= centre.entries.length)
            centre.currentIndex = centre.entries.length - 1;
        else if (centre.currentIndex < 0 && centre.entries.length > 0)
            centre.currentIndex = 0;
    }

    function send(verb, options) {
        if (centre.providerSource)
            centre.providerSource.sendCommand("notifications", verb, options);
    }

    function dismiss(index) {
        if (index < 0 || index >= centre.live.length)
            return;
        centre.send("dismiss", {"id": centre.live[index].id});
    }

    // Only a live notification can still be acted on: a producer that already
    // withdrew its notification is not waiting for an answer to it.
    function invokeFirst(index) {
        if (index < 0 || index >= centre.live.length)
            return;
        const entry = centre.live[index];
        const offered = centre.actionsFor(entry.id);
        if (offered.length === 0)
            return;
        centre.send("invoke", {"id": entry.id, "action": offered[0].key});
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
        onPressed: centre.dismissed()
    }

    Item {
        id: scene

        width: centre.cardWidth
        height: centre.cardHeight
        anchors.centerIn: parent

        // Anything the card itself does not handle stops here rather than
        // falling through to the dismissal area behind it. It is the first
        // child, so every control declared after it is still reached first.
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        }

        GlassCard {
            anchors.fill: parent
            backdropSource: scene
            Accessible.role: Accessible.Dialog
            Accessible.name: qsTr("Notificaciones")

            Column {
                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceLg
                spacing: CelestinaTheme.spaceMd

                Row {
                    width: parent.width
                    spacing: CelestinaTheme.spaceSm

                    Text {
                        width: parent.width - quietButton.width - clearButton.width
                               - parent.spacing * 2
                        anchors.verticalCenter: parent.verticalCenter
                        text: centre.quiet ? qsTr("Notificaciones — silenciadas")
                                           : qsTr("Notificaciones")
                        color: CelestinaTheme.text
                        elide: Text.ElideRight
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowTitle
                        font.weight: CelestinaTheme.weightDemiBold
                    }

                    CelestinaButton {
                        id: quietButton

                        text: centre.quiet ? qsTr("Permitir") : qsTr("Silenciar")
                        role: centre.quiet ? CelestinaButton.Selected
                                           : CelestinaButton.Tonal
                        helpText: qsTr("Retener las notificaciones salvo las críticas (D)")
                        onClicked: centre.send("quiet-toggle", {})
                    }

                    CelestinaButton {
                        id: clearButton

                        text: qsTr("Vaciar")
                        role: CelestinaButton.Destructive
                        enabled: centre.past.length > 0
                        helpText: qsTr("Olvidar lo que ya terminó")
                        onClicked: centre.send("clear-history", {})
                    }
                }

                ListView {
                    id: list

                    width: parent.width
                    height: parent.height - y
                    clip: true
                    spacing: CelestinaTheme.spaceXs
                    model: centre.entries
                    currentIndex: centre.currentIndex
                    keyNavigationEnabled: true
                    Accessible.role: Accessible.List

                    onCurrentIndexChanged: centre.currentIndex = currentIndex

                    Keys.onPressed: (event) => {
                        if (event.key === Qt.Key_Return
                                   || event.key === Qt.Key_Enter) {
                            centre.invokeFirst(centre.currentIndex);
                            event.accepted = true;
                        } else if (event.key === Qt.Key_Delete
                                   || event.key === Qt.Key_Backspace) {
                            centre.dismiss(centre.currentIndex);
                            event.accepted = true;
                        } else if (event.key === Qt.Key_D) {
                            centre.send("quiet-toggle", {});
                            event.accepted = true;
                        }
                    }

                    delegate: Item {
                        id: row

                        required property int index
                        required property var modelData

                        readonly property bool live: row.index < centre.live.length
                        readonly property bool selected: row.index === centre.currentIndex

                        width: ListView.view.width
                        implicitHeight: rowBody.implicitHeight + CelestinaTheme.spaceMd

                        Accessible.role: Accessible.ListItem
                        // Chained because QML's `arg` substitutes one value per
                        // call, unlike its C++ namesake. A producer that puts a
                        // `%2` in its own app name can garble this sentence but
                        // cannot reach past it.
                        Accessible.name: qsTr("%1: %2. %3")
                            .arg(row.modelData.app)
                            .arg(row.modelData.summary)
                            .arg(row.modelData.body)
                        Accessible.selected: row.selected

                        Rectangle {
                            anchors.fill: parent
                            radius: CelestinaTheme.radiusSm
                            color: row.selected ? CelestinaTheme.surfaceSelected
                                                : CelestinaTheme.clear
                        }

                        Column {
                            id: rowBody

                            width: parent.width - CelestinaTheme.spaceMd
                            x: CelestinaTheme.spaceSm
                            y: CelestinaTheme.spaceXs
                            spacing: 2

                            Text {
                                width: parent.width
                                // What has ended is shown quieter than what is
                                // still live, so the list says which is which
                                // without a second column of labels.
                                text: row.live
                                      ? row.modelData.app
                                      : qsTr("%1 — terminada").arg(row.modelData.app)
                                // Producer text is shown as characters, never
                                // interpreted as markup: `AutoText` would guess
                                // otherwise and render a link, or an image this
                                // shell would fetch on the producer's behalf.
                                // The server never advertises `body-markup`.
                                textFormat: Text.PlainText
                                color: CelestinaTheme.textMuted
                                elide: Text.ElideRight
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontCaption
                            }

                            Text {
                                width: parent.width
                                text: row.modelData.summary
                                textFormat: Text.PlainText
                                color: row.live ? CelestinaTheme.text
                                                : CelestinaTheme.textMuted
                                elide: Text.ElideRight
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontBody
                                font.weight: row.modelData.read
                                             ? CelestinaTheme.weightRegular
                                             : CelestinaTheme.weightDemiBold
                            }

                            Text {
                                width: parent.width
                                visible: row.modelData.body.length > 0
                                text: row.modelData.body
                                textFormat: Text.PlainText
                                color: CelestinaTheme.textMuted
                                wrapMode: Text.WordWrap
                                maximumLineCount: 2
                                elide: Text.ElideRight
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontCaption
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: centre.currentIndex = row.index
                        }
                    }
                }

                Text {
                    width: parent.width
                    visible: !centre.serving || centre.entries.length === 0
                             || centre.truncated
                    text: !centre.serving
                          ? qsTr("Otro programa es el servidor de notificaciones de esta sesión, así que este shell no tiene nada que mostrar.")
                          : centre.entries.length === 0
                            ? qsTr("No se ha dicho nada últimamente.")
                            : qsTr("Las notificaciones más antiguas no se conservan.")
                    color: CelestinaTheme.textMuted
                    wrapMode: Text.WordWrap
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                }
            }
        }
    }
}
