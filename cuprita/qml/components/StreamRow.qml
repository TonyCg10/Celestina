pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0

// One application playing or recording: its name, its own volume and mute.
Item {
    id: row

    // The StreamModel row: id, appName, appIcon, volume, percent, muted.
    required property var model
    required property int index

    signal volumeRequested(int id, real volume)
    signal muteRequested(int id, bool muted)

    implicitHeight: CelestinaTheme.rowHeight
    Accessible.role: Accessible.ListItem
    Accessible.name: row.model.appName

    CelestinaIcon {
        id: glyph
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: CelestinaTheme.iconMd
        height: CelestinaTheme.iconMd
        name: row.model.appIcon
        fallbackName: "music"
    }

    Text {
        id: label
        anchors.left: glyph.right
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width / 3
        text: row.model.appName
        elide: Text.ElideRight
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowTitle
    }

    CelestinaSlider {
        anchors.left: label.right
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.right: percent.left
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        value: row.model.percent
        to: 150
        step: 5
        Accessible.name: qsTr("Volumen de %1").arg(row.model.appName)
        onMoved: function(value) { row.volumeRequested(row.model.id, value / 100) }
    }

    Text {
        id: percent
        anchors.right: mute.left
        anchors.rightMargin: CelestinaTheme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        text: qsTr("%1 %").arg(row.model.percent)
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowSecondary
    }

    CelestinaIconButton {
        id: mute
        objectName: "muteButton"
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        role: CelestinaButton.Ghost
        iconName: row.model.muted ? "media-volume-muted" : "media-volume"
        helpText: row.model.muted ? qsTr("Activar sonido") : qsTr("Silenciar")
        onClicked: row.muteRequested(row.model.id, !row.model.muted)
    }
}
