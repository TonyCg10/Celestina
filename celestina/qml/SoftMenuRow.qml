// PANEL-1. One semantic item in a real Qt Quick menu.
//
// `GlassMenuItem` remains the lifecycle and accessibility owner. This file
// supplies the Control Centre's visual hierarchy without predicting state:
// icon/status at the leading edge, title/subtitle in the centre, and the
// provider-confirmed state or action at the trailing edge.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

GlassMenuItem {
    id: row

    required property BackdropInk ink
    property bool actionable: true
    property bool header: false
    property bool sectionLabel: false
    property bool separator: false
    property string iconName: ""
    property string fallbackIcon: iconName
    // StatusNotifierItems are the one accepted exception to the shell's
    // closed first-party icon catalogue. The URL is already resolved and
    // bounded by the tray host; an empty or failed URL falls back to a
    // catalogue icon.
    property url iconSource: ""
    property bool secondaryActionable: false
    property bool contextActionable: false
    property string subtitle: ""
    property string note: ""
    property color noteColor: row.ink.faint
    property color dot: CelestinaTheme.clear
    // Optional icon actions occupy the row's trailing column without giving
    // any shared semantic meaning to them. The owning menu names and handles
    // the actions; this component owns only their consistent anatomy.
    property string trailingPrimaryIcon: ""
    property string trailingPrimaryHelpText: ""
    property bool trailingPrimarySelected: false
    property bool trailingPrimaryEnabled: true
    property string trailingSecondaryIcon: ""
    property string trailingSecondaryHelpText: ""
    property bool trailingSecondarySelected: false
    property bool trailingSecondaryEnabled: true
    // These opt-in dimensions distinguish outer section rhythm from the
    // air inside a row. The header's trailing space remains transparent so it
    // separates the two glass sections instead of making either one denser.
    property int headerTrailingGap: 0
    property int verticalInset: 0
    property int trailingGap: 0
    readonly property bool hasExternalIcon: row.iconSource.toString().length > 0
    readonly property bool externalIconReady: row.hasExternalIcon
                                               && externalIcon.status === Image.Ready
    readonly property bool externalIconFailed: row.hasExternalIcon
                                                && externalIcon.status === Image.Error
    readonly property string effectiveIconName: row.iconName.length > 0
                                                ? row.iconName : row.fallbackIcon
    readonly property bool hasTrailingActions: row.trailingPrimaryIcon.length > 0
                                               || row.trailingSecondaryIcon.length > 0
    readonly property int visualHeight: row.header
                                         ? CelestinaTheme.rowHeight
                                           + CelestinaTheme.borderFocus
                                           + CelestinaTheme.spaceSm
                                         : row.separator
                                           ? CelestinaTheme.spaceMd
                                           : row.subtitle.length > 0
                                             ? CelestinaTheme.controlHeightXl
                                               + row.verticalInset * 2
                                             : CelestinaTheme.controlHeightXs
                                               + CelestinaTheme.spaceXs
                                               + row.verticalInset * 2

    signal secondaryTriggered(int globalX, int globalY)
    signal contextTriggered(int globalX, int globalY)
    signal trailingPrimaryTriggered()
    signal trailingSecondaryTriggered()

    enabled: row.actionable || row.hasTrailingActions
    implicitWidth: CelestinaTheme.compMenuWidth
                   + CelestinaTheme.space3xl * 3
                   - CelestinaTheme.compMenuPadding * 2
    implicitHeight: row.visualHeight
                    + (row.header ? row.headerTrailingGap : row.trailingGap)
    leftPadding: row.header ? 0 : CelestinaTheme.spaceMd
    rightPadding: row.header ? 0 : CelestinaTheme.spaceMd

    Accessible.role: row.enabled ? Accessible.MenuItem : Accessible.StaticText
    Accessible.name: {
        let description = row.text;
        if (row.subtitle.length > 0)
            description = qsTr("%1, %2").arg(description).arg(row.subtitle);
        if (row.note.length > 0)
            description = qsTr("%1, %2").arg(description).arg(row.note);
        return description;
    }
    Accessible.ignored: row.separator

    contentItem: Item {
        Loader {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: row.visualHeight
            active: row.header

            sourceComponent: Component {
                MenuHeader {
                    ink: row.ink
                    title: row.text
                    subtitle: row.subtitle
                    iconName: row.iconName
                    fallbackIcon: row.fallbackIcon
                    compact: true
                }
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: -row.trailingGap / 2
            height: CelestinaTheme.borderHairline
            visible: row.separator
            color: row.ink.divider
        }

        Item {
            id: leadingSlot

            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: -row.trailingGap / 2
            width: CelestinaTheme.iconSm
            height: width
            visible: !row.header && !row.separator
                     && (row.hasExternalIcon || row.iconName.length > 0
                         || row.dot.a > 0)

            Image {
                id: externalIcon

                objectName: "celestina-menu-row-external-icon"
                anchors.fill: parent
                visible: row.externalIconReady
                source: row.iconSource
                sourceSize.width: CelestinaTheme.iconSm
                sourceSize.height: CelestinaTheme.iconSm
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                smooth: true
            }

            CelestinaIcon {
                objectName: "celestina-menu-row-fallback-icon"
                anchors.fill: parent
                visible: !row.externalIconReady
                         && row.effectiveIconName.length > 0
                name: row.effectiveIconName
                fallbackName: row.fallbackIcon.length > 0
                              ? row.fallbackIcon : row.effectiveIconName
                tintOverride: row.current ? row.ink.accent
                                          : (row.actionable ? row.ink.primary
                                                            : row.ink.muted)
                Accessible.ignored: true
            }

            Rectangle {
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                width: CelestinaTheme.compStatusIndicatorSize
                height: width
                radius: width / 2
                visible: row.dot.a > 0
                color: row.dot
            }
        }

        Column {
            anchors.left: leadingSlot.visible ? leadingSlot.right : parent.left
            anchors.leftMargin: leadingSlot.visible ? CelestinaTheme.spaceSm : 0
            anchors.right: noteLabel.visible ? noteLabel.left
                           : trailingActions.visible ? trailingActions.left
                           : parent.right
            anchors.rightMargin: noteLabel.visible || trailingActions.visible
                                 ? CelestinaTheme.spaceSm : 0
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: -row.trailingGap / 2
            spacing: CelestinaTheme.spaceXs
            visible: !row.header && !row.separator

            Text {
                width: parent.width
                text: row.text
                textFormat: Text.PlainText
                color: row.sectionLabel ? row.ink.muted
                       : row.enabled ? row.ink.primary : row.ink.faint
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: row.sectionLabel ? CelestinaTheme.fontRowSecondary
                                                 : row.actionable
                                                   ? CelestinaTheme.fontBody
                                                   : CelestinaTheme.fontMini
                font.weight: row.current || row.sectionLabel
                             ? CelestinaTheme.weightDemiBold
                             : CelestinaTheme.weightRegular
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                visible: row.subtitle.length > 0
                text: row.subtitle
                textFormat: Text.PlainText
                color: row.ink.muted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
                elide: Text.ElideRight
            }
        }

        Text {
            id: noteLabel

            anchors.right: trailingActions.visible
                           ? trailingActions.left : parent.right
            anchors.rightMargin: trailingActions.visible
                                 ? CelestinaTheme.spaceSm : 0
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: -row.trailingGap / 2
            visible: !row.header && !row.separator && row.note.length > 0
            text: row.note
            textFormat: Text.PlainText
            color: row.noteColor
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontMini
            font.weight: CelestinaTheme.weightDemiBold
        }

        Row {
            id: trailingActions

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: -row.trailingGap / 2
            spacing: CelestinaTheme.spaceXs
            visible: row.hasTrailingActions && !row.header && !row.separator
            z: 2

            BackdropIconButton {
                objectName: "celestina-menu-row-primary-action"
                visible: row.trailingPrimaryIcon.length > 0
                width: CelestinaTheme.controlHeightXs
                height: CelestinaTheme.controlHeightXs
                ink: row.ink
                iconName: row.trailingPrimaryIcon
                helpText: row.trailingPrimaryHelpText
                enabled: row.trailingPrimaryEnabled
                role: row.trailingPrimarySelected
                      ? CelestinaButton.Selected : CelestinaButton.Ghost
                onClicked: row.trailingPrimaryTriggered()
            }

            BackdropIconButton {
                objectName: "celestina-menu-row-secondary-action"
                visible: row.trailingSecondaryIcon.length > 0
                width: CelestinaTheme.controlHeightXs
                height: CelestinaTheme.controlHeightXs
                ink: row.ink
                iconName: row.trailingSecondaryIcon
                helpText: row.trailingSecondaryHelpText
                enabled: row.trailingSecondaryEnabled
                role: row.trailingSecondarySelected
                      ? CelestinaButton.Selected : CelestinaButton.Ghost
                onClicked: row.trailingSecondaryTriggered()
            }
        }
    }

    background: Item {
        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.bottomMargin: row.trailingGap
            visible: !row.header && !row.separator
                     && (row.actionable || row.hasTrailingActions)
                     && (row.down || secondaryPointer.pressed || row.hovered
                         || row.highlighted || row.current)
            radius: CelestinaTheme.radiusSm
            color: {
                if (row.down || secondaryPointer.pressed)
                    return row.ink.selectedFill;

                if (row.current)
                    return row.ink.accentFill;

                return row.ink.hoverFill;
            }
            border.width: 0
            border.color: CelestinaTheme.clear

            Behavior on color {
                enabled: !CelestinaTheme.reducedMotion

                ColorAnimation {
                    duration: CelestinaTheme.motionFast
                    easing.type: CelestinaTheme.easeStandard
                }
            }
        }
    }

    // Left click and keyboard activation remain MenuItem's primary action.
    // These handlers add only the two StatusNotifierItem pointer routes and
    // ignore left presses, so the real Menu keeps its grab and lifecycle.
    MouseArea {
        id: secondaryPointer

        anchors.fill: parent
        anchors.bottomMargin: row.trailingGap
        enabled: row.secondaryActionable || row.contextActionable
        acceptedButtons: (row.secondaryActionable ? Qt.MiddleButton : Qt.NoButton)
                         | (row.contextActionable ? Qt.RightButton : Qt.NoButton)
        cursorShape: Qt.PointingHandCursor
        onClicked: (mouse) => {
            const at = row.mapToGlobal(0, row.height);
            if (mouse.button === Qt.MiddleButton)
                row.secondaryTriggered(at.x, at.y);
            else if (mouse.button === Qt.RightButton)
                row.contextTriggered(at.x, at.y);
        }
    }
}
