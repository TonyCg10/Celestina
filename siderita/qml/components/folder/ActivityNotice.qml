pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

    // ── One transient announcement ────────────────────────────────────
    // A pill as wide as its line, never as wide as the window. It has two
    // tenses and is the same object in both: it is born with a turning dot
    // while the action runs, and mutates into a word when the action settles.
    //
    // A ring means "this is running and you can stop it". This means "this
    // happened", which is why a 300 ms unmount gets one of these and not a ring
    // that would appear and vanish inside one blink. The 500 ms threshold below
    // is what makes that true: an action that settles inside it is never drawn.
Item {
    id: notice

    required property string noticeId
    required property string text
    required property string iconName
    required property bool danger
    required property bool running
    // The widest this may be: the content frame minus the capsule and its gaps.
    required property real maxWidth
    property Item backdrop
    property bool floating: true

    // Nothing appears before this. It is the whole reason a short action is
    // safe to announce, and it is measured from the moment the notice arrives.
    readonly property int appearAfter: 500
    // How long a settled notice stays. An error stays long enough to be read on
    // a long path; a press always ends either sooner.
    readonly property int dwell: notice.danger ? 10000 : 4000
    // An error may wrap; an announcement is one line. An elided error is an
    // error a person cannot read, which is the one thing the full-width banners
    // this replaces did right.
    readonly property int lines: notice.danger ? 2 : 1

    // Drawn only once the threshold has been earned. Read by the stack and the
    // tests; `drawn` is this component's own and nothing outside assigns it.
    readonly property bool shown: notice.drawn
    property bool drawn: false
    property bool retiring: false

    signal dismissed(string id)

    // Retires now: plays the one exit if it was ever drawn, and otherwise just
    // asks to be dropped, because there is nothing on screen to take away.
    function retire() {
        if (notice.retiring)
            return
        if (!notice.drawn) {
            notice.dismissed(notice.noticeId)
            return
        }
        notice.retiring = true
        exit.restart()
    }

    function show() {
        notice.drawn = true
        entry.restart()
    }

    readonly property real textRoom: notice.maxWidth - glyph.width
                                     - 2 * CelestinaTheme.spaceMd
                                     - CelestinaTheme.spaceSm

    implicitWidth: Math.min(notice.maxWidth,
                            glyph.width + label.width
                            + 2 * CelestinaTheme.spaceMd
                            + CelestinaTheme.spaceSm)
    implicitHeight: Math.max(CelestinaTheme.controlHeightSm,
                             label.implicitHeight + 2 * CelestinaTheme.spaceSm)
    width: implicitWidth
    height: implicitHeight
    visible: notice.drawn
    opacity: 0

    // An alert is announced without being focused, which is what a surface that
    // retires by itself needs: a screen reader hears it, and nothing steals the
    // keyboard from the folder. Escape dismisses the danger ones, in the stack.
    Accessible.role: Accessible.AlertMessage
    Accessible.name: notice.text

    // ── The two clocks ────────────────────────────────────────────────
    // Neither of them animates anything: the one animated gesture is the exit
    // below, which has a single clock of its own.
    Timer {
        id: appearClock
        interval: notice.appearAfter
        running: notice.running && !notice.drawn && !notice.retiring
        onTriggered: notice.show()
    }

    Timer {
        interval: notice.dwell
        running: notice.drawn && !notice.running && !notice.retiring
        onTriggered: notice.retire()
    }

    // A notice that arrives already settled is an announcement about something
    // that has happened. Delaying it would only make the application feel slow.
    Component.onCompleted: if (!notice.running) notice.show()

    onRunningChanged: {
        if (notice.running)
            return
        if (!notice.drawn) {
            // It settled inside the threshold: never drawn, and gone. The dwell
            // clock must not get a turn, or a 200 ms unmount would appear after
            // the fact.
            notice.dismissed(notice.noticeId)
        }
        // Otherwise it was drawn while it ran, its wording has already changed
        // with the model, and the dwell clock takes over by itself.
    }

    // One fade in, one fade-and-shrink out: the suite's single exit, one clock
    // per gesture, no second animation racing it.
    NumberAnimation {
        id: entry
        target: notice
        property: "opacity"
        to: 1
        duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionExit
        easing.type: CelestinaTheme.easeStandard
    }

    ParallelAnimation {
        id: exit
        NumberAnimation {
            target: notice
            property: "opacity"
            to: 0
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionExit
            easing.type: CelestinaTheme.easeExit
        }
        NumberAnimation {
            target: notice
            property: "scale"
            to: CelestinaTheme.exitShrink
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionExit
            easing.type: CelestinaTheme.easeExit
        }
        onFinished: notice.dismissed(notice.noticeId)
    }

    GlassPill {
        anchors.fill: parent
        backdrop: notice.backdrop
        floating: notice.floating
        fill: notice.danger ? CelestinaTheme.dangerFill
                            : CelestinaTheme.controlFill
        border.width: notice.danger ? CelestinaTheme.borderHairline : 0
        border.color: CelestinaTheme.dangerBorder
        // The area below turns the swallowed press into a dismissal instead of
        // a dead zone, so the pill's own floor would only be in its way.
        inputShield: false
    }

    // A press retires it. That is what makes swallowing the click honest: the
    // notice covers a row, so the press must not select or open that row, and a
    // press that did nothing at all would be a dead patch of window.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        hoverEnabled: true
        preventStealing: true
        onPressed: notice.retire()
    }

    Item {
        id: glyph
        width: CelestinaTheme.iconSm
        height: width
        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter

        CelestinaIcon {
            anchors.fill: parent
            name: notice.iconName
            fallbackName: notice.iconName
            tone: notice.danger ? CelestinaIcon.Danger : CelestinaIcon.Primary
        }

        // While the action runs the glyph carries a turning mark; when it
        // settles the mark goes and the wording has already changed. One
        // object, two tenses.
        Rectangle {
            id: spinner
            visible: notice.running
            width: 6
            height: width
            radius: width / 2
            color: CelestinaTheme.accent
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            RotationAnimator on rotation {
                running: spinner.visible && !CelestinaTheme.reducedMotion
                loops: Animation.Infinite
                from: 0
                to: 360
                duration: CelestinaTheme.motionSlow * 6
            }
        }
    }

    Text {
        id: label
        width: Math.min(implicitWidth, notice.textRoom)
        anchors.left: glyph.right
        anchors.leftMargin: CelestinaTheme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        text: notice.text
        color: notice.danger ? CelestinaTheme.dangerFillInk : CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        wrapMode: notice.lines > 1 ? Text.Wrap : Text.NoWrap
        maximumLineCount: notice.lines
        elide: Text.ElideRight
    }
}
