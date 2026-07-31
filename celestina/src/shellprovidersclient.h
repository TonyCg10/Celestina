#pragma once

#include <QElapsedTimer>
#include <QObject>
#include <QProcess>
#include <QTimer>
#include <QVariantMap>

#include "protocoldecoder.h"
#include "providerstates.h"

// The panel's one bridge to the aggregate provider helper.
//
// There is exactly one of these, not one per widget: the helper carries every
// provider that needs long-lived, non-Qt IO, and this class owns its process,
// its framing and the marshaling of confirmed state onto the GUI thread. A
// widget receives narrow scalars from `providers()`; it never learns that a
// process exists.
//
// This remains manual C++ for the same reason `NiriClient` does: CXX-Qt 0.9 has
// no supported CMake-side generator that can join this LayerShellQt target. The
// class is deliberately limited to process lifecycle, bounded validation and Qt
// marshaling — every rule about what a provider value means lives in
// `celestina-shell-core`.
class ShellProvidersClient final : public QObject
{
    Q_OBJECT
    Q_PROPERTY(bool available READ available NOTIFY changed)
    Q_PROPERTY(QVariantMap providers READ providers NOTIFY changed)

public:
    explicit ShellProvidersClient(QObject *parent = nullptr);
    ~ShellProvidersClient() override;

    bool available() const { return m_available; }
    QVariantMap providers() const { return m_states.providers(); }

    // Asks a provider to do something. Returns 0 when the request could not
    // even be sent; otherwise the id its result will carry. Acceptance is not
    // arrival — what a command did shows up in that provider's next value.
    Q_INVOKABLE qulonglong sendCommand(
        const QString &provider,
        const QString &verb,
        const QVariantMap &options = QVariantMap()
    );

signals:
    void changed();
    void commandResult(
        qulonglong requestId,
        const QString &state,
        const QString &reason
    );

private slots:
    void readStandardOutput();
    void readStandardError();
    void helperStopped(int exitCode, QProcess::ExitStatus exitStatus);
    void helperError(QProcess::ProcessError error);

private:
    void startHelper();
    void scheduleRestart();
    void applyLine(const QByteArray &line);
    void setUnavailable();

    QProcess m_process;
    QTimer m_restartTimer;
    ProtocolDecoder m_decoder;
    ProviderStates m_states;
    // Ids are never reused, so a result from a previous helper process can
    // never answer a request made to the current one.
    quint64 m_lastRequestId = 0;
    bool m_available = false;
    bool m_stopping = false;
    int m_restartDelayMs = 0;
};
