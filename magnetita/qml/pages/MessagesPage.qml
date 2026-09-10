// The delegates below reach the page's `root` id, which a delegate may only
// do under bound component behaviour.
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.magnetita 1.0

// The phone's SMS: the conversation list, then one thread with a field to
// answer. Everything comes from the daemon, which asks the phone; the page
// re-reads while it is on screen, since the daemon signals no SMS change
// of its own.
Item {
    id: root

    required property MessagesModel messages
    required property string deviceId

    readonly property bool threadOpen: root.messages.openThread.length > 0

    onDeviceIdChanged: {
        root.messages.deviceId = root.deviceId
        root.messages.refresh()
    }

    Timer {
        interval: 3000
        running: root.visible && root.deviceId.length > 0
        repeat: true
        triggeredOnStart: true
        onTriggered: root.messages.refresh()
    }

    // ── The list ─────────────────────────────────────────────────────────
    ScrollPage {
        anchors.fill: parent
        visible: !root.threadOpen
        spacing: 2

        Text {
            width: parent.width
            visible: root.messages.conversationThreads.length === 0
            text: root.deviceId.length === 0
                  ? qsTr("Sin teléfono conectado.")
                  : root.messages.available
                    ? qsTr("Sin conversaciones todavía. El móvil las envía cuando la app tiene permiso de SMS.")
                    : qsTr("El servicio Magnetita no está disponible.")
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            wrapMode: Text.WordWrap
        }

        Repeater {
            model: root.messages.conversationThreads

            delegate: ConversationRow {
                required property int index
                required property string modelData

                width: parent.width
                label: index < root.messages.conversationLabels.length ? root.messages.conversationLabels[index] : modelData
                snippet: index < root.messages.conversationSnippets.length ? root.messages.conversationSnippets[index] : ""
                unread: index < root.messages.conversationUnread.length ? root.messages.conversationUnread[index] : "0"
                onOpenRequested: root.messages.openConversation(modelData, label)
            }
        }
    }

    // ── One thread ───────────────────────────────────────────────────────
    ColumnLayout {
        anchors.fill: parent
        visible: root.threadOpen
        spacing: CelestinaTheme.spaceSm

        RowLayout {
            Layout.fillWidth: true
            spacing: CelestinaTheme.spaceSm

            CelestinaIconButton {
                iconName: "chevron-left"
                helpText: qsTr("Volver a las conversaciones")
                onClicked: root.messages.closeConversation()
            }

            Text {
                Layout.fillWidth: true
                text: root.messages.openLabel
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.weight: CelestinaTheme.weightDemiBold
                elide: Text.ElideRight
            }
        }

        ListView {
            id: thread
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: CelestinaTheme.spaceXs
            model: root.messages.messageBodies
            onCountChanged: positionViewAtEnd()

            delegate: MessageBubble {
                required property int index
                required property string modelData

                width: thread.width
                body: modelData
                fromMe: index < root.messages.messageFromMe.length && root.messages.messageFromMe[index] === "true"
                name: index < root.messages.messageNames.length ? root.messages.messageNames[index] : ""
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: CelestinaTheme.spaceSm

            CelestinaTextField {
                id: reply
                Layout.fillWidth: true
                placeholderText: qsTr("Escribe un mensaje")
                onAccepted: sendButton.clicked()
            }

            CelestinaIconButton {
                id: sendButton
                iconName: "corner-down-left"
                role: CelestinaButton.Primary
                enabled: reply.text.trim().length > 0
                helpText: qsTr("Enviar")
                onClicked: {
                    root.messages.send(reply.text)
                    reply.text = ""
                }
            }
        }
    }
}
