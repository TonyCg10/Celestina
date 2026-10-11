import QtQuick
import org.celestina.selenita 1.0

// The corner preview (capture preview design §4): after every capture saved
// to a file and every finished recording, the result shows in this small
// frameless window, which niri's rule (matching Selenita's app id and this
// title) places at the bottom-right corner without the focus. Dragging it
// offers the file as a copy, a click hands the file to Fluorita's editor,
// the × or five seconds alone close it and the pointer resting on it holds
// it. It never takes the keyboard focus: it is a pointer affordance, and
// everything it does is in the history too. What it shows is the
// controller's; the window only presents it and tells the controller when
// it goes.
Window {
    id: preview
    objectName: "previewWindow"

    // How long the preview stays with nobody on it.
    readonly property int holdTime: 5000
    readonly property int previewWidth: 240
    readonly property int lowestHeight: 140
    readonly property int highestHeight: 240
    // A recording without a poster has no picture to measure: the screen's
    // usual shape.
    readonly property real fallbackAspect: 9 / 16
    readonly property real aspect: SelenitaController.previewAspect > 0
                                   ? SelenitaController.previewAspect
                                   : preview.fallbackAspect
    readonly property bool recording: SelenitaController.previewKind === "recording"
    // A picture or a poster to show; a recording without a poster (or a
    // file that does not load) shows the film glyph instead.
    readonly property bool hasPicture: picture.status !== Image.Null
                                       && picture.status !== Image.Error
    // How far below its place the card starts as it enters.
    readonly property int slide: CelestinaTheme.spaceLg
    readonly property int motion: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionNormal
    // For the tests: whether the pointer rests on it and whether the five
    // seconds run.
    readonly property alias hovered: hover.hovered
    readonly property alias holding: hold.running

    // Shows the current result from the start: a new one replaces the one
    // on screen and its five seconds start again.
    function enter() {
        exit.stop()
        preview.visible = true
        if (CelestinaTheme.reducedMotion) {
            entry.stop()
            card.opacity = 1
            card.offset = 0
        } else {
            entry.restart()
        }
        preview.holdOrRelease()
    }

    // Hides at once, with no exit: the main window is closing and Qt must
    // find no window left on screen, so the process ends.
    function hideNow() {
        hold.stop()
        entry.stop()
        exit.stop()
        card.opacity = 0
        preview.visible = false
    }

    // Fades out and hides; nothing when already hidden.
    function leave() {
        hold.stop()
        entry.stop()
        if (!preview.visible)
            return
        if (CelestinaTheme.reducedMotion) {
            card.opacity = 0
            preview.visible = false
        } else if (!exit.running) {
            exit.restart()
        }
    }

    // The five seconds run only while nobody holds the preview: the pointer
    // on it or a drag out of it stops them, and they start again from the
    // beginning when it leaves.
    function holdOrRelease() {
        if (hover.hovered || drag.active || !preview.visible || exit.running)
            hold.stop()
        else
            hold.restart()
    }

    title: qsTr("Vista previa")
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowDoesNotAcceptFocus
    // Its own top-level window: not a dialog of the main window, which may
    // be hidden (a key-binding launch never shows it).
    transientParent: null
    visible: false
    width: preview.previewWidth
    height: Math.max(preview.lowestHeight,
                     Math.min(preview.highestHeight,
                              Math.round(preview.previewWidth * preview.aspect)))
    // Transparent: the compositor blurs what lies behind it and the
    // backdrop lays the canvas over it, as the main window does.
    color: CelestinaTheme.clear

    // Closed by the compositor while it shows, it is dismissed like the ×
    // does, so a key-binding launch with nothing else on screen ends its
    // own way (a recording it took afterwards is not cut short). Hidden, it
    // lets the application's quit close it.
    onClosing: (close) => {
        if (preview.visible) {
            close.accepted = false
            SelenitaController.dismissPreview()
        }
    }

    Connections {
        target: SelenitaController

        function onPreviewShown() {
            preview.enter()
        }

        function onPreviewVisibleChanged() {
            if (!SelenitaController.previewVisible)
                preview.leave()
        }
    }

    Timer {
        id: hold
        interval: preview.holdTime
        onTriggered: SelenitaController.dismissPreview()
    }

    ParallelAnimation {
        id: entry

        NumberAnimation {
            target: card
            property: "opacity"
            from: 0
            to: 1
            duration: preview.motion
            easing.type: CelestinaTheme.easeStandard
        }

        NumberAnimation {
            target: card
            property: "offset"
            from: preview.slide
            to: 0
            duration: preview.motion
            easing.type: CelestinaTheme.easeDecelerate
        }
    }

    SequentialAnimation {
        id: exit

        ParallelAnimation {
            NumberAnimation {
                target: card
                property: "opacity"
                to: 0
                duration: preview.motion
                easing.type: CelestinaTheme.easeStandard
            }

            NumberAnimation {
                target: card
                property: "offset"
                to: preview.slide
                duration: preview.motion
                easing.type: CelestinaTheme.easeStandard
            }
        }

        ScriptAction {
            script: preview.visible = false
        }
    }

    Item {
        id: card
        objectName: "previewCard"

        // The card's distance below its place, for the entry and the exit.
        property real offset: 0

        anchors.fill: parent
        opacity: 0
        transform: Translate {
            y: card.offset
        }
        Accessible.role: Accessible.Graphic
        Accessible.name: preview.title

        // The file itself, offered as a copy: a drop target that asks to
        // move it is refused. Qt makes the URI one line of a `text/uri-list`.
        Drag.active: drag.active
        Drag.dragType: Drag.Automatic
        Drag.supportedActions: Qt.CopyAction
        Drag.proposedAction: Qt.CopyAction
        Drag.mimeData: ({ "text/uri-list": SelenitaController.previewFileUri })
        Drag.imageSource: SelenitaController.previewSource
        Drag.imageSourceSize: Qt.size(preview.previewWidth / 2,
                                      Math.round(preview.previewWidth / 2 * preview.aspect))

        CelestinaBackdrop {
            anchors.fill: parent
        }

        Rectangle {
            id: frame
            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceSm
            radius: CelestinaTheme.radiusMd
            color: CelestinaTheme.controlFill
            clip: true

            Image {
                id: picture
                objectName: "previewPicture"
                anchors.fill: parent
                visible: preview.hasPicture
                source: SelenitaController.previewSource
                sourceSize.width: preview.previewWidth * 2
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: false
                Accessible.ignored: true
            }

            // A recording whose first frame could not be taken: the film
            // glyph stands for it.
            CelestinaIcon {
                objectName: "previewPlaceholder"
                anchors.centerIn: parent
                visible: !preview.hasPicture
                name: "film"
                width: CelestinaTheme.iconLg
                height: width
                tone: CelestinaIcon.Secondary
                Accessible.ignored: true
            }

            // A recording's glyph and length, in a pill over the picture.
            Rectangle {
                anchors.left: parent.left
                anchors.bottom: parent.bottom
                anchors.margins: CelestinaTheme.spaceSm
                visible: preview.recording
                width: length.implicitWidth + CelestinaTheme.spaceSm * 2
                height: length.implicitHeight + CelestinaTheme.spaceXs
                radius: CelestinaTheme.radiusPill
                color: CelestinaTheme.pillFill

                Row {
                    id: length
                    anchors.centerIn: parent
                    spacing: CelestinaTheme.spaceXs

                    CelestinaIcon {
                        objectName: "previewFilm"
                        anchors.verticalCenter: parent.verticalCenter
                        visible: preview.recording
                        name: "film"
                        width: CelestinaTheme.iconSm
                        height: width
                        Accessible.ignored: true
                    }

                    Text {
                        objectName: "previewDuration"
                        anchors.verticalCenter: parent.verticalCenter
                        visible: preview.recording && text !== ""
                        text: SelenitaController.previewDuration
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                        font.features: { "tnum": 1 }
                    }
                }
            }

            // Why Fluorita did not open, said where the person looked.
            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                // Concentric with the frame: radiusSm + spaceXs = radiusMd.
                anchors.margins: CelestinaTheme.spaceXs
                visible: SelenitaController.previewNotice !== ""
                height: notice.implicitHeight + CelestinaTheme.spaceXs * 2
                radius: CelestinaTheme.radiusSm
                color: CelestinaTheme.pillFill

                Text {
                    id: notice
                    objectName: "previewNotice"
                    anchors.fill: parent
                    anchors.leftMargin: CelestinaTheme.spaceSm
                    anchors.rightMargin: CelestinaTheme.spaceSm
                    verticalAlignment: Text.AlignVCenter
                    text: SelenitaController.previewNotice
                    color: CelestinaTheme.danger
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    wrapMode: Text.Wrap
                    Accessible.role: Accessible.AlertMessage
                    Accessible.name: notice.text
                }
            }
        }

        HoverHandler {
            id: hover
            onHoveredChanged: preview.holdOrRelease()
        }

        // A drag that leaves the threshold behind is the file going out; the
        // card itself stays where it is.
        DragHandler {
            id: drag
            target: null
            onActiveChanged: preview.holdOrRelease()
        }

        TapHandler {
            onTapped: {
                SelenitaController.editPreview()
                preview.leave()
            }
        }

        CelestinaIconButton {
            objectName: "previewClose"
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: CelestinaTheme.spaceSm
            iconName: "x"
            role: CelestinaButton.Tonal
            helpText: qsTr("Cerrar la vista previa")
            focusPolicy: Qt.NoFocus
            onClicked: SelenitaController.dismissPreview()
        }
    }
}
