pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// One systemd unit: a dot for its state, its name and description, and which
// manager it belongs to. Content family: the shared row highlight paints
// hover, press and selection.
AbstractButton {
    id: row

    required property string unitName
    required property string unitDescription
    // `system` or `user`.
    required property string scope
    // The unit's active state, as systemd words it.
    required property string active
    required property bool selected

    readonly property string scopeWord: row.scope === "system" ? qsTr("Sistema") : qsTr("Usuario")
    readonly property string stateWord: {
        switch (row.active) {
        case "active": return qsTr("activo")
        case "inactive": return qsTr("inactivo")
        case "failed": return qsTr("fallido")
        case "activating": return qsTr("arrancando")
        case "deactivating": return qsTr("parando")
        case "reloading": return qsTr("recargando")
        }
        return row.active
    }
    // systemd's description reads better than a unit's file name, so it leads
    // when it says something the name does not; the name is then the second
    // line, and otherwise the only one.
    readonly property bool descriptionLeads: row.unitDescription.length > 0
                                             && row.unitDescription !== row.unitName
    readonly property string primaryText: row.descriptionLeads ? row.unitDescription : row.unitName
    readonly property string secondaryText: row.descriptionLeads ? row.unitName : ""

    implicitHeight: CelestinaTheme.rowHeight
    hoverEnabled: true
    // The list is the one Tab stop; see `ProcessRow`.
    focusPolicy: Qt.NoFocus

    Accessible.role: Accessible.ListItem
    Accessible.name: (row.descriptionLeads ? row.unitDescription + ", " : "")
                     + row.unitName + ", " + row.scopeWord + ", " + row.stateWord
    Accessible.selected: row.selected

    background: CelestinaRowHighlight {
        family: CelestinaRowHighlight.Content
        hovered: row.hovered
        pressed: row.down
        selected: row.selected
        focused: row.visualFocus
    }

    contentItem: Item {
        Row {
            anchors.fill: parent
            anchors.leftMargin: CelestinaTheme.spaceMd
            anchors.rightMargin: CelestinaTheme.spaceMd
            spacing: CelestinaTheme.spaceMd

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.compStatusIndicatorSize
                height: width
                radius: width / 2
                color: row.active === "active" ? CelestinaTheme.success
                     : row.active === "failed" ? CelestinaTheme.danger
                     : (row.active === "activating" || row.active === "deactivating"
                        || row.active === "reloading") ? CelestinaTheme.warning
                     : CelestinaTheme.textFaint
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - chip.width - CelestinaTheme.compStatusIndicatorSize
                       - parent.spacing * 3

                Text {
                    text: row.primaryText
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowTitle
                    elide: Text.ElideRight
                    width: parent.width
                }

                Text {
                    visible: row.secondaryText.length > 0
                    text: row.secondaryText
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowSecondary
                    elide: Text.ElideRight
                    width: parent.width
                }
            }

            Rectangle {
                id: chip

                anchors.verticalCenter: parent.verticalCenter
                width: scopeLabel.implicitWidth + CelestinaTheme.spaceMd
                height: scopeLabel.implicitHeight + CelestinaTheme.spaceXs
                radius: CelestinaTheme.radiusPill
                color: CelestinaTheme.badgeFill

                Text {
                    id: scopeLabel

                    anchors.centerIn: parent
                    text: row.scopeWord
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                }
            }
        }
    }
}
