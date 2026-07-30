#pragma once

#include <QObject>
#include <QPointer>
#include <QSize>
#include <QTimer>

class QWindow;

// Owns the compositor-blur lifecycle for one layer-shell window.
//
// This remains manual C++ because KWindowEffects' Wayland surface integration
// is not available through CXX-Qt. The controller is deliberately limited to
// effect capability, surface geometry and the QML fallback flag.
class PanelBlurController final : public QObject
{
public:
    explicit PanelBlurController(QWindow *window, QObject *parent = nullptr);

    void start();

private:
    enum class State {
        Pending,
        Enabled,
        Fallback,
    };

    void geometryChanged();
    void probe();
    void scheduleProbe(int delayMs);
    void setAvailable(bool available);

    QPointer<QWindow> m_window;
    QTimer m_probeTimer;
    QSize m_armedSize;
    State m_state = State::Pending;
    int m_fastAttemptsRemaining = 0;
};
