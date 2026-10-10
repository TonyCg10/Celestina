import QtQuick
import org.celestina.selenita 1.0

// A labelled switch row. The switch mirrors `checked`; a click reports the
// wish through `toggled` and the binding is restored, so the switch only
// stays moved once the owner of the value agrees.
Item {
    id: row

    required property string label
    // The card's `rowInset`.
    required property int inset
    property bool checked: false
    // A muted line under the label saying why the switch is as it is.
    property string hint: ""
    // A hairline above, when a row of the same card comes before this one.
    property bool separated: false

    signal toggled(bool on)

    width: parent ? parent.width : 0
    implicitHeight: row.hint.length > 0 ? CelestinaTheme.rowHeightLg : CelestinaTheme.rowHeight

    RowDivider {
        inset: row.inset
        visible: row.separated
    }

    HoverHandler { id: hover }

    CelestinaRowHighlight {
        anchors.fill: parent
        radius: CelestinaTheme.radiusMd
        family: CelestinaRowHighlight.Content
        hovered: hover.hovered && row.enabled
    }

    Column {
        anchors.left: parent.left
        anchors.leftMargin: row.inset
        anchors.right: toggle.left
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter

        // The switch reads the label and the hint; the words are not read
        // twice.
        Text {
            width: parent.width
            text: row.label
            elide: Text.ElideRight
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            Accessible.ignored: true
        }

        Text {
            objectName: "settingHint"
            width: parent.width
            visible: row.hint.length > 0
            text: row.hint
            elide: Text.ElideRight
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            Accessible.ignored: true
        }
    }

    CelestinaSwitch {
        id: toggle
        objectName: "settingSwitch"
        anchors.right: parent.right
        anchors.rightMargin: row.inset
        anchors.verticalCenter: parent.verticalCenter
        checked: row.checked
        enabled: row.enabled
        Accessible.role: Accessible.CheckBox
        Accessible.name: row.label
        Accessible.description: row.hint
        onToggled: {
            row.toggled(toggle.checked)
            toggle.checked = Qt.binding(() => row.checked)
        }
    }
}
