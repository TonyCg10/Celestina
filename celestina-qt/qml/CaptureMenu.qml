// Shell tools exposed through the panel's real contextual-menu path.
//
// Two tools with deliberately different shapes. A capture is a request and
// nothing more: Niri owns the screenshot UI and the save destination, and the
// panel reports only a request it could not send. A recording is a state the
// session is *in* — it outlives this menu, it has a running clock, and it
// writes a file — so it is read from the provider that owns the recorder,
// never from what this surface last asked for.
//
// Starting one asks which screen first, through the session's own chooser, so
// this row raises the question instead of answering it: the menu is open on
// one output but a person recording a bug is not always looking at that one.
// Stopping needs no question and goes straight to the provider.
//
// Both close the menu on activation, and for the same reason: whatever the
// menu is covering is exactly what the person is trying to capture.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import "ProviderReading.js" as ProviderReading

SoftMenu {
    id: root

    required property var providerSource

    signal captureRequested()
    // Asks the panel for the screen chooser. The menu never starts a recording
    // itself: it does not know which output the person means.
    signal recordRequested()

    readonly property var recorder: ProviderReading.read(root.providerSource,
                                                         "recorder")
    readonly property bool canRecord: root.recorder !== undefined
                                      && root.recorder.available === true
    readonly property bool recording: root.recorder !== undefined
                                      && root.recorder.recording === true
    // Why the last attempt did nothing. The helper names a cause; the sentence
    // is this shell's own, in the words the person reads.
    readonly property string recordFailure: {
        if (root.recorder === undefined || root.recorder.failure === undefined)
            return "";

        if (root.recorder.failure === "start-failed")
            return qsTr("no se pudo empezar a grabar");

        if (root.recorder.failure === "close-failed")
            return qsTr("la última grabación quedó sin cerrar");

        return qsTr("falló");
    }
    // Milliseconds since the recording started, or 0. Kept as the provider's
    // own instant rather than a count this surface increments: a menu that is
    // closed and opened again must show the true elapsed time, not restart it.
    readonly property real recordingSince: root.recording
                                           && root.recorder.since !== undefined
                                           ? root.recorder.since : 0

    // What makes the label tick: re-read once a second while the menu is up,
    // and never otherwise. This is the only clock in the shell that has to
    // move without a provider frame behind it.
    property real nowMs: 0

    Timer {
        interval: 1000
        repeat: true
        running: root.recording && root.menu.visible
        triggeredOnStart: true
        onTriggered: root.nowMs = Date.now()
    }

    function elapsedText() {
        // Whichever is later: the tick is what re-evaluates this, but the
        // elapsed time is measured now — which is also what makes the first
        // frame after the menu opens right, before any tick has landed.
        const now = Math.max(root.nowMs, Date.now());
        const seconds = Math.max(
            0, Math.floor((now - root.recordingSince) / 1000));
        const minutes = Math.floor(seconds / 60);
        return qsTr("%1:%2").arg(minutes)
                            .arg(String(seconds % 60).padStart(2, "0"));
    }

    title: qsTr("Caja de herramientas")
    itemSpacing: CelestinaTheme.spaceSm
    headerBodyGap: CelestinaTheme.spaceMd
    rowVerticalInset: CelestinaTheme.spaceXs

    // Keep one ordered stream under the real Menu owner. Later capture modes
    // can add entries without mixing static children and model delegates.
    readonly property var entries: [
        {"kind": "header", "text": qsTr("Caja de herramientas"),
         "subtitle": qsTr("Herramientas disponibles")},
        {"kind": "section", "text": qsTr("Herramientas")},
        {"kind": "capture", "text": qsTr("Capturar pantalla")},
        {"kind": "record"}
    ]

    // Deliberately not a field of the entry above: this changes once a second
    // while a recording runs, and rebuilding the model would take every row of
    // the real Menu down and put it back with it. Only this label moves.
    readonly property string recordLabel:
        root.recording
        ? qsTr("Detener la grabación · %1").arg(root.elapsedText())
        : (!root.canRecord
           ? qsTr("Grabar pantalla · sin grabador")
           : (root.recordFailure.length > 0
              ? qsTr("Grabar una pantalla… · %1").arg(root.recordFailure)
              : qsTr("Grabar una pantalla…")))

    function requestCapture() {
        root.captureRequested();
        root.menu.close();
    }

    // The provider owns whether this session is recording. Stopping is said
    // straight to it; starting is a question this surface cannot answer, so it
    // is handed to the panel, which owns the chooser.
    function requestRecording() {
        if (!root.canRecord)
            return;

        if (root.recording) {
            if (root.providerSource)
                root.providerSource.sendCommand("recorder", "record-stop", {});
        } else {
            root.recordRequested();
        }
        root.menu.close();
    }

    Instantiator {
        model: root.entries
        onObjectAdded: (index, object) => root.menu.insertItem(index, object)
        onObjectRemoved: (index, object) => root.menu.removeItem(object)

        delegate: SoftMenuRow {
            id: entry

            required property var modelData

            readonly property bool isHeader: entry.modelData.kind === "header"
            readonly property bool isSection: entry.modelData.kind === "section"
            readonly property bool isCapture: entry.modelData.kind === "capture"
            readonly property bool isRecord: entry.modelData.kind === "record"

            ink: root.ink
            text: entry.isRecord ? root.recordLabel : entry.modelData.text
            header: entry.isHeader
            sectionLabel: entry.isSection
            subtitle: entry.isHeader ? entry.modelData.subtitle : ""
            iconName: entry.isHeader ? "toolbox"
                                      : entry.isCapture ? "scissors"
                                        : entry.isRecord ? "film" : ""
            // A session with no recorder still says so, in the row where the
            // tool would have been, rather than hiding what it cannot do.
            actionable: entry.isCapture || (entry.isRecord && root.canRecord)
            headerTrailingGap: entry.isHeader ? root.headerBodyGap : 0
            verticalInset: root.rowVerticalInset
            trailingGap: entry.isHeader ? 0 : root.itemSpacing
            onTriggered: {
                if (entry.isCapture)
                    root.requestCapture();
                else if (entry.isRecord)
                    root.requestRecording();
            }
        }
    }
}
