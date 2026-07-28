import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import org.celestina.siderita 1.0

    // ── "Abrir con…" application chooser ─────────────────────────────
Rectangle {
    id: openWithView
    property var controller
    property var owner
    property var backdrop   // mainPanel: el fondo que difumina el cristal
    anchors.fill: parent
    z: 66
    readonly property bool shown: controller.openWithPending
    // Fades rather than pops. Opacity only: a scale transform on a
    // glass surface desyncs its backdrop sampling (see a995619), so the
    // motion here never touches geometry.
    visible: opacity > 0.01
    opacity: shown ? 1 : 0
    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.motionFast
            easing.type: CelestinaTheme.easeStandard
        }
    }
    color: CelestinaTheme.scrim

    property int selected: -1
    readonly property int appCount: controller.openWithApps.length
    onVisibleChanged: if (visible) selected = controller.openWithDefaultIndex
    onSelectedChanged: if (selected >= 0)
                           openWithList.positionViewAtIndex(
                               selected, ListView.Contain)

    MouseArea {
        anchors.fill: parent
        onClicked: controller.cancelOpenWith()
    }
    // Fully keyboard-operable: arrows move the selection, Enter opens
    // it, Escape cancels.
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            controller.cancelOpenWith()
            event.accepted = true
        } else if (event.key === Qt.Key_Down) {
            if (openWithView.appCount > 0)
                openWithView.selected = Math.min(
                    openWithView.appCount - 1, openWithView.selected + 1)
            event.accepted = true
        } else if (event.key === Qt.Key_Up) {
            if (openWithView.appCount > 0)
                openWithView.selected = Math.max(
                    0, (openWithView.selected < 0 ? 0 : openWithView.selected) - 1)
            event.accepted = true
        } else if ((event.key === Qt.Key_Return
                    || event.key === Qt.Key_Enter)
                   && openWithView.selected >= 0) {
            controller.openWithApp(openWithView.selected, false)
            event.accepted = true
        }
    }
    focus: openWithView.shown

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(480, owner.width - 48)
        height: Math.min(420, owner.height - 64)
        backdropSource: openWithView.backdrop
        // (not transform-scaled — a scale transform desynced the glass backdrop)
        Accessible.role: Accessible.Dialog
        Accessible.name: "Abrir con"

        MouseArea { anchors.fill: parent }

        Text {
            id: openWithHeading
            x: 18
            y: 16
            width: parent.width - 36
            text: "Abrir «" + controller.openWithTarget + "» con"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
            elide: Text.ElideRight
        }

        Text {
            anchors.centerIn: parent
            visible: controller.openWithApps.length === 0
            text: "No hay aplicaciones que declaren este tipo"
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
        }

        ListView {
            id: openWithList
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.top: openWithHeading.bottom
            anchors.topMargin: 12
            anchors.bottom: openWithButtons.top
            anchors.bottomMargin: 12
            clip: true
            spacing: 2
            model: controller.openWithApps

            delegate: Item {
                id: appRow
                required property int index
                required property string modelData
                width: ListView.view.width
                height: 38
                Accessible.role: Accessible.ListItem
                Accessible.name: appRow.modelData
                Accessible.selected: openWithView.selected === appRow.index

                Rectangle {
                    anchors.fill: parent
                    radius: CelestinaTheme.radiusSm
                    color: openWithView.selected === appRow.index
                           ? CelestinaTheme.badgeAccentFill
                           : appRowMouse.containsMouse
                             ? CelestinaTheme.surfaceHover : "transparent"
                }

                Text {
                    x: 12
                    anchors.verticalCenter: parent.verticalCenter
                    width: defaultBadge.visible
                           ? defaultBadge.x - x - 8 : parent.width - x - 12
                    text: appRow.modelData
                    color: openWithView.selected === appRow.index
                           ? CelestinaTheme.accent : CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowSecondary
                    elide: Text.ElideRight
                }

                Text {
                    id: defaultBadge
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.right: parent.right
                    anchors.rightMargin: 12
                    visible: controller.openWithDefaultIndex === appRow.index
                    text: "predeterminada"
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontMini
                }

                MouseArea {
                    id: appRowMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: openWithView.selected = appRow.index
                    onDoubleClicked: controller.openWithApp(appRow.index, false)
                }
            }
        }

        Row {
            id: openWithButtons
            anchors.right: parent.right
            anchors.rightMargin: 18
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 16
            spacing: 8

            CelestinaButton {
                text: "Cancelar"
                onClicked: controller.cancelOpenWith()
            }
            CelestinaButton {
                text: "Predeterminar y abrir"
                enabled: openWithView.selected >= 0
                onClicked: controller.openWithApp(openWithView.selected, true)
            }
            CelestinaButton {
                text: "Abrir"
                primary: true
                enabled: openWithView.selected >= 0
                onClicked: controller.openWithApp(openWithView.selected, false)
            }
        }
    }
}
