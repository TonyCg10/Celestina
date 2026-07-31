#pragma once

#include <QByteArray>
#include <QObject>
#include <QProcess>
#include <QTimer>
#include <QVariantList>

#include "protocoldecoder.h"

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
    QVariantList workspaces() const { return m_workspaces; }

signals:
    void changed();

private slots:
    void readStandardOutput();
    void readStandardError();
    void adapterStopped(int exitCode, QProcess::ExitStatus exitStatus);
    void adapterError(QProcess::ProcessError error);

private:
    void startAdapter();
    void scheduleRestart();
    void applyMessage(const QByteArray &line);
    void setUnavailable();

    QProcess m_process;
    QTimer m_restartTimer;
    NiriProtocolDecoder m_decoder;
    QVariantList m_workspaces;
    bool m_available = false;
    bool m_stopping = false;
    int m_restartDelayMs = 0;
};
