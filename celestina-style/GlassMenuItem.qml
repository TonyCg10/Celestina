import QtQuick
import QtQuick.Controls

// ─── GlassMenuItem ────────────────────────────────────────────────────────────
// A MenuItem styled for GlassContextMenu: optional leading icon or colour
// swatch, `current` mark, submenu chevron and the suite's state treatment.
// ──────────────────────────────────────────────────────────────────────────────
MenuItem {
    id: control

    property bool current: false
    // Marks a mutually-exclusive choice without handing state mutation to
    // MenuItem.checkable. The caller owns `current`; the shared component owns
    // its visual and accessibility representation.
    property bool choice: false
    property bool showSwatch: false
    property bool automaticSwatch: false
    property color swatchColor: CelestinaTheme.clear
    readonly property bool hasIcon: icon.name.length > 0
                                    || icon.source.toString().length > 0

    Accessible.checkable: choice || checkable
    Accessible.checked: choice ? current : checked

    implicitWidth: CelestinaTheme.compMenuWidth - CelestinaTheme.compMenuPadding * 2
    implicitHeight: CelestinaTheme.controlHeight
    leftPadding: CelestinaTheme.spaceMd
    rightPadding: CelestinaTheme.spaceMd
    topPadding: CelestinaTheme.spaceSm
    bottomPadding: CelestinaTheme.spaceSm

    // Draw both affordances inside the content item. The platform Basic style
    // otherwise leaks its bitmap check and arrow into an otherwise Lucide menu.
    indicator: Item {
        implicitWidth: 0
        implicitHeight: 0
    }
    arrow: Item {
        implicitWidth: 0
        implicitHeight: 0
    }

    contentItem: Item {
        Item {
            id: leadingSlot
            width: CelestinaTheme.iconSm
            height: CelestinaTheme.iconSm
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            visible: control.showSwatch || control.hasIcon
            opacity: control.enabled ? 1 : CelestinaTheme.disabledContentOpacity

            CelestinaIcon {
                anchors.fill: parent
                visible: !control.showSwatch && control.hasIcon
                name: control.icon.name
                fallbackName: CelestinaIcons.keyFromSource(control.icon.source)
                tone: control.current ? CelestinaIcon.Accent
                                      : control.enabled
                                        ? CelestinaIcon.Primary
                                        : CelestinaIcon.Secondary
            }

            Rectangle {
                id: swatch
                anchors.centerIn: parent
                width: 14
                height: 14
                radius: width / 2
                visible: control.showSwatch
                color: control.automaticSwatch
                       ? CelestinaTheme.clear : control.swatchColor
                border.width: CelestinaTheme.borderHairline
                border.color: control.automaticSwatch
                              ? CelestinaTheme.textMuted
                              : CelestinaTheme.withAlpha(
                                    CelestinaTheme.text, 0.24)

                Rectangle {
                    anchors.centerIn: parent
                    visible: control.automaticSwatch
                    width: 11
                    height: CelestinaTheme.borderHairline
                    radius: height / 2
                    rotation: -45
                    color: CelestinaTheme.textMuted
                }
            }
        }

        CelestinaIcon {
            id: submenuChevron
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.iconSm
            height: CelestinaTheme.iconSm
            visible: control.subMenu !== null
            name: "chevron-right"
            fallbackName: "chevron-right"
            tone: CelestinaIcon.Secondary
            opacity: control.enabled ? 1 : CelestinaTheme.disabledContentOpacity
        }

        CelestinaIcon {
            id: currentMark
            anchors.right: submenuChevron.visible
                           ? submenuChevron.left : parent.right
            anchors.rightMargin: submenuChevron.visible
                                 ? CelestinaTheme.spaceXs : 0
            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.iconSm
            height: CelestinaTheme.iconSm
            visible: control.current || (control.checkable && control.checked)
            name: "check"
            fallbackName: "check"
            tone: CelestinaIcon.Accent
            opacity: control.enabled ? 1 : CelestinaTheme.disabledContentOpacity
        }

        Text {
            anchors.left: leadingSlot.visible ? leadingSlot.right : parent.left
            anchors.leftMargin: leadingSlot.visible ? CelestinaTheme.spaceSm : 0
            anchors.right: currentMark.visible
                           ? currentMark.left
                           : submenuChevron.visible
                             ? submenuChevron.left : parent.right
            anchors.rightMargin: currentMark.visible || submenuChevron.visible
                                 ? CelestinaTheme.spaceSm : 0
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            text: control.text
            color: control.current
                   ? CelestinaTheme.accent
                   : control.enabled
                     ? CelestinaTheme.text
                     : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
            opacity: control.enabled ? 1 : CelestinaTheme.disabledContentOpacity
        }
    }

    background: Rectangle {
        radius: CelestinaTheme.radiusSm
        color: control.highlighted || control.current
               ? CelestinaTheme.surfaceHover
               : CelestinaTheme.clear

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
            }
        }
    }
}
