#pragma once

#include <QByteArray>
#include <QElapsedTimer>
#include <QObject>
#include <QProcess>
#include <QSet>
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
    // Asks Niri to open its own screenshot UI. Unlike a workspace focus there
    // is nothing later to confirm it against — the compositor takes over the
    // screen — so the panel reports only that the request could not be made.
    Q_INVOKABLE qulonglong requestScreenshot();

signals:
    void changed();
    // One transition of one request: "pending", then "confirmed" or "failed".
    void focusRequestChanged(qulonglong requestId, const QString &state);
    void screenshotFailed(const QString &reason);

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
    bool applySnapshot(const QJsonObject &root);
    bool applyRequestResult(const QJsonObject &root);
    void setUnavailable();
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
    // The screenshot requests still waiting for an answer. They are not
    // workspace requests: nothing confirms them, so they are only remembered
    // long enough to report a refusal against the right button.
    QSet<quint64> m_screenshotRequests;
    // The compositor's snapshot as validated, before request state is merged
    // into the list QML consumes.
    QVariantList m_snapshot;
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
