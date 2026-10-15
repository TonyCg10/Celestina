#pragma once

#include <QByteArray>
#include <QElapsedTimer>
#include <QHash>
#include <QList>
#include <QObject>
#include <QProcess>
#include <QTimer>
#include <QVariantList>

#include "protocoldecoder.h"
#include "workspacefocusrequests.h"

class QJsonObject;

// Thin GUI-thread adapter for the Rust Niri helper.
//
// This remains manual C++ because CXX-Qt 0.9 has no supported CMake-side code
// generator that can join this existing LayerShellQt CMake target. The class is
// intentionally limited to process lifecycle, bounded JSON validation and Qt
// marshaling; Niri protocol/state/reconnection remain in Rust.
class NiriClient final : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool available READ available NOTIFY changed)
    Q_PROPERTY(QVariantList workspaces READ workspaces NOTIFY changed)

public:
    explicit NiriClient(QObject *parent = nullptr);
    ~NiriClient() override;

    bool available() const { return m_available; }
    // Each entry carries the compositor's own state plus `requestState`: the
    // panel's view of a focus request it asked for, never a predicted result.
    QVariantList workspaces() const { return m_workspaces; }

    // Asks Niri to focus a workspace the last snapshot actually contained.
    // Returns 0 when the shell cannot even send the request; otherwise the id
    // of a request that is pending, and whose life is reported through
    // `workspaces()` and `focusRequestChanged`.
    Q_INVOKABLE qulonglong requestWorkspaceFocus(const QString &output, int index);
    // Focus one window by the id a snapshot published. Not a tracked request:
    // see the implementation for why a map has nothing to report an outcome to.
    Q_INVOKABLE bool requestWindowFocus(const QString &windowId);
    // Asks Niri to open its own screenshot UI. Unlike a workspace focus there
    // is nothing later to confirm it against — the compositor takes over the
    // screen — so the panel reports only that the request could not be made.
    Q_INVOKABLE qulonglong requestScreenshot();
    // The output whose workspace currently holds focus, or an empty string when nothing
    // says. The focused window lives there, so it is also the monitor whose bubble anchor a
    // minimize should travel to.
    Q_INVOKABLE QString focusedOutput() const;
    // Outputs whose active workspace holds a fullscreen window, by the
    // compositor's own output names — the one tenant every parked shell
    // surface yields direct scanout to (SURF-1-C). Empty when nothing is
    // fullscreen, when the helper is unavailable, or when it predates the
    // field; every one of those answers means "yield nothing".
    QStringList fullscreenOutputs() const { return m_fullscreenOutputs; }
    // Asks Niri to blank the outputs. Unlike a workspace focus, the shell sees
    // no later snapshot that could confirm it: the compositor's own answer to
    // the request is the outcome, and it is reported as such.
    qulonglong requestDisplaysOff();
    // Asks Niri to end the session. Its own answer is the outcome.
    qulonglong requestLogOut();

signals:
    void changed();
    // The fullscreen tenancy of some output changed. Emitted beside `changed`
    // so parked-surface owners can react without diffing whole snapshots.
    void fullscreenOutputsChanged();
    // One transition of one request: "pending", then "confirmed" or "failed".
    void focusRequestChanged(qulonglong requestId, const QString &state);
    void screenshotFailed(const QString &reason);
    // One compositor action whose result Niri itself reported: "confirmed" or
    // "failed", once, with the compositor's reason when it refused.
    void actionFinished(
        qulonglong requestId,
        const QString &state,
        const QString &reason
    );

private slots:
    void readStandardOutput();
    void readStandardError();
    void adapterStopped(int exitCode, QProcess::ExitStatus exitStatus);
    void adapterError(QProcess::ProcessError error);
    void expireRequests();
    void drainRequestOutcomes();

private:
    void startAdapter();
    void scheduleRestart();
    void applyMessage(const QByteArray &line);
    // Sends one `kind`-tagged request line to the adapter. Returns 0 when it
    // could not even be written.
    qulonglong sendRequest(const QString &kind);
    bool applySnapshot(const QJsonObject &root);
    bool applyRequestResult(const QJsonObject &root);
    void setUnavailable();
    // Reports the named screenshot requests as refused and forgets them.
    // Whether the answer was lost with the helper or never came at all, the
    // button that asked must learn the same way it learns about a real refusal.
    void failScreenshotRequests(const QList<quint64> &requestIds, const QString &reason);
    // The same for the actions, which additionally name their outcome to
    // whoever asked over the session channel.
    void failActionRequests(const QList<quint64> &requestIds, const QString &reason);
    // Runs the deadline sweep while anything is in flight. Every request kind
    // arms it, because every request kind can now be expired by it.
    void startRequestSweep();
    // Rebuilds the list QML consumes from the last snapshot and the current
    // request states. Returns whether that list changed; it never emits, so
    // one caller decides when a single `changed()` is due.
    bool rebuildWorkspaces();
    // Emits one `focusRequestChanged` per recorded transition, one event-loop
    // turn after the change that produced it.
    void scheduleRequestOutcomes();
    qint64 nowMs() const { return m_clock.elapsed(); }

    QProcess m_process;
    QTimer m_restartTimer;
    QTimer m_requestTimer;
    QElapsedTimer m_clock;
    ProtocolDecoder m_decoder;
    WorkspaceFocusRequests m_requests;
    // The screenshot requests still waiting for an answer, each against the
    // millisecond it was sent. They are not workspace requests: nothing
    // confirms them, so they are only remembered long enough to report a
    // refusal — or a silence — against the right button.
    QHash<quint64, qint64> m_screenshotRequests;
    // Actions whose outcome is reported to whoever asked. Nothing confirms
    // them later either, but unlike a screenshot the requester is told when
    // the compositor did carry one out.
    QHash<quint64, qint64> m_actionRequests;
    // The compositor's snapshot as validated, before request state is merged
    // into the list QML consumes.
    QVariantList m_snapshot;
    QStringList m_fullscreenOutputs;
    QVariantList m_workspaces;
    // Ids are never reused, and every request records the adapter process it
    // belongs to, so a result from a previous helper cannot answer a new
    // request.
    quint64 m_lastRequestId = 0;
    quint64 m_generation = 0;
    bool m_outcomesQueued = false;
    bool m_available = false;
    bool m_stopping = false;
    int m_restartDelayMs = 0;
};
