import QtQuick
import org.celestina.siderita 1.0

// Centered One UI heading: the location's title, which the scroll retires.
// FolderView owns the gesture; this component only draws how far it has gone.
// It used to grow into a metadata summary on a push up at the top; the author
// never reached for it, and the frame it moved under the rows was where a fast
// scroll jumped, so the title is the whole heading now.
Item {
    id: root

    required property var controller
    required property var hostWindow
    required property bool shortcutActive
    // How far the heading has gone, 0…1. It arrives already interpolated from
    // the scroll travel (`HeadingScroll`); this component only draws it. At
    // rest, with nothing retired, a folder shows its title: the mistake that
    // preceded all this left the window with no title at all and the rows
    // sliding under the bars.
    required property real retiredProgress
    signal phoneMediaRequested(int index)

    readonly property int phoneIndex: {
        if (controller.trashActive || controller.recentActive
                || controller.searchActive || controller.searchRunning)
            return -1
        controller.phoneRevision
        const current = controller.currentPathKey.replace(/\/+$/, "")
        for (let index = 0; index < controller.phoneMounts.length; ++index) {
            const mount = controller.phoneMounts[index].replace(/\/+$/, "")
            if (mount.length > 0 && mount === current)
                return index
        }
        return -1
    }
    readonly property bool phoneLocation: phoneIndex >= 0
    readonly property var phoneInfo: {
        controller.phoneRevision
        return phoneLocation ? controller.phoneInfo(phoneIndex) : []
    }
    readonly property bool phoneConnected:
            phoneInfo.length > 3 && phoneInfo[3] === "1"

    // No Behavior on it: the gesture is the clock. An animation between two
    // scroll-driven values would only run behind the finger.

    readonly property string locationName: {
        if (controller.trashActive)
            return "Papelera"
        if (controller.recentActive)
            return "Recientes"
        if (controller.searchActive || controller.searchRunning)
            return "Resultados"
        // Re-evaluate when Magnetita republishes its parallel device lists.
        controller.phoneNames.length
        controller.phoneMounts.length
        // ADR 0008: the label comes from the adapter, which owns both the
        // decode and the phone-name substitution. No path arithmetic here.
        if (controller.currentPathKey.length === 0)
            return "Inicio"
        return controller.displayLocationName(controller.currentPathKey)
    }

    readonly property real restingHeight: 60
    height: Math.round(restingHeight * (1 - retiredProgress))
    opacity: 1 - retiredProgress
    visible: opacity > 0.01
    // Nothing may spill out of a band that is closing.
    clip: true

    Column {
        x: root.phoneLocation ? 12 : 6
        y: (parent.height - implicitHeight) / 2
        width: root.phoneLocation
               ? Math.max(0, parent.width - mediaButton.width - 42)
               : Math.max(0, parent.width - 12)
        Text {
            width: parent.width
            text: root.locationName.toUpperCase()
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontHeaderCollapsed
                                       * root.hostWindow.interfaceTextScale)
            font.weight: CelestinaTheme.weightDemiBold
            elide: Text.ElideMiddle
            horizontalAlignment: root.phoneLocation
                                 ? Text.AlignLeft : Text.AlignHCenter
        }

        // A lost watch is not an event: it stays true of this folder for as
        // long as it lasts, so it lives where the folder describes itself and
        // not in a column that retires by itself. It used to sit in the bottom
        // strip, which carried it alongside four kinds of transient message.
        Row {
            id: watchWarning
            height: visible ? implicitHeight : 0
            visible: root.controller.watchDegraded
            spacing: CelestinaTheme.spaceSm
            // Centred like the rest of the heading, except on a phone, where
            // the heading aligns left. Placed by x rather than by an anchor:
            // anchors are not how a positioner's children are arranged.
            x: root.phoneLocation ? 0 : (parent.width - width) / 2

            CelestinaIcon {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.round(CelestinaTheme.iconSm
                                  * root.hostWindow.interfaceIconScale)
                height: width
                name: "circle-alert"
                fallbackName: "circle-alert"
                tone: CelestinaIcon.Danger
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("Vigilancia perdida · instantánea")
                color: CelestinaTheme.dangerFillInk
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: Math.round(CelestinaTheme.fontCaption
                                           * root.hostWindow.interfaceTextScale)
            }
        }
    }

    PhoneMediaButton {
        id: mediaButton

        anchors.right: parent.right
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        // Handed over to the path bar only once the heading itself is gone,
        // halfway through the retirement in both directions. It fades with
        // the heading instead of vanishing under the pointer mid-scroll.
        readonly property bool carried: root.retiredProgress < 0.5
        visible: root.phoneLocation
        enabled: root.phoneLocation && carried
        opacity: carried ? 1 : 0

        Behavior on opacity {
            NumberAnimation {
                duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }
        connected: root.phoneConnected
        onClicked: root.phoneMediaRequested(root.phoneIndex)
    }

    Shortcut {
        sequence: "Alt+M"
        enabled: root.shortcutActive && root.phoneLocation && root.phoneConnected
        onActivated: root.phoneMediaRequested(root.phoneIndex)
    }
}
