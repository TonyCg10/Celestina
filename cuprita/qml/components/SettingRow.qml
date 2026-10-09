import QtQuick
import org.celestina.cuprita 1.0

// One labelled switch: the row carries the words, the switch the state. The
// switch shows what the controller says; a click asks for the change and the
// binding comes back, so the switch only stays moved once the backend agrees.
Item {
    id: root

    required property string label
    property bool checked: false
    // What a screen reader calls the switch; the label unless a row says more.
    property string accessibleName: root.label
    // A muted line under the label saying why the switch is as it is.
    property string hint: ""
    // A hairline above the row, when another row of the card precedes it.
    property bool separated: false

    signal toggled(bool on)

    // The card's `rowInset`, from whoever places the row in a SectionCard.
    required property int inset

    implicitHeight: CelestinaTheme.rowHeight

    RowDivider { inset: root.inset; visible: root.separated }

    HoverHandler { id: hover }

    CelestinaRowHighlight {
        anchors.fill: parent
        radius: CelestinaTheme.radiusMd
        family: CelestinaRowHighlight.Content
        hovered: hover.hovered && root.enabled
    }

    Column {
        anchors.left: parent.left
        anchors.leftMargin: root.inset
        anchors.right: toggle.left
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter

        Text {
            width: parent.width
            text: root.label
            elide: Text.ElideRight
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
        }

        Text {
            objectName: "settingHint"
            width: parent.width
            visible: root.hint.length > 0
            text: root.hint
            elide: Text.ElideRight
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }
    }

    CelestinaSwitch {
        id: toggle
        objectName: "settingSwitch"
        anchors.right: parent.right
        anchors.rightMargin: root.inset
        anchors.verticalCenter: parent.verticalCenter
        checked: root.checked
        Accessible.role: Accessible.CheckBox
        Accessible.name: root.accessibleName
        onToggled: {
            root.toggled(toggle.checked)
            toggle.checked = Qt.binding(function() { return root.checked })
        }
    }
}
