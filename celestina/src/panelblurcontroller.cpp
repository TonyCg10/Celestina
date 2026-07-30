#include "panelblurcontroller.h"

#include <QDebug>
#include <QRegion>
#include <QVariant>
#include <QWindow>

#include <KWindowEffects>

namespace {
constexpr int fastProbeDelayMs = 16;
constexpr int fastProbeCount = 60;
constexpr int fallbackProbeDelayMs = 2000;
constexpr int enabledProbeDelayMs = 5000;
}

PanelBlurController::PanelBlurController(QWindow *window, QObject *parent)
    : QObject(parent)
    , m_window(window)
{
    m_probeTimer.setSingleShot(true);
    QObject::connect(&m_probeTimer, &QTimer::timeout, this, [this] { probe(); });
}

void PanelBlurController::start()
{
    if (!m_window)
        return;

    QObject::connect(
        m_window.data(), &QWindow::widthChanged, this,
        [this] { geometryChanged(); }
    );
    QObject::connect(
        m_window.data(), &QWindow::heightChanged, this,
        [this] { geometryChanged(); }
    );
    QObject::connect(
        m_window.data(), &QWindow::screenChanged, this,
        [this] { geometryChanged(); }
    );
    QObject::connect(
        m_window.data(), &QWindow::visibleChanged, this,
        [this](bool visible) {
            if (visible)
                geometryChanged();
        }
    );

    geometryChanged();
}

void PanelBlurController::geometryChanged()
{
    m_armedSize = {};
    m_state = State::Pending;
    m_fastAttemptsRemaining = fastProbeCount;
    m_probeTimer.stop();
    probe();
}

void PanelBlurController::probe()
{
    if (!m_window)
        return;

    const bool surfaceReady = m_window->isVisible()
        && m_window->isExposed()
        && !m_window->size().isEmpty();
    const bool effectAvailable = KWindowEffects::isEffectAvailable(
        KWindowEffects::BlurBehind
    );

    if (surfaceReady && effectAvailable) {
        if (m_state != State::Enabled || m_armedSize != m_window->size()) {
            // ext-background-effect state is double-buffered. Submit a finite,
            // surface-local region and request the frame that commits it.
            KWindowEffects::enableBlurBehind(
                m_window.data(),
                true,
                QRegion(QRect(QPoint(0, 0), m_window->size()))
            );
            m_window->requestUpdate();
            m_armedSize = m_window->size();
            qInfo() << "Celestina compositor blur armed on"
                    << m_window->objectName() << "at" << m_armedSize;
        }
        m_state = State::Enabled;
        setAvailable(true);
        scheduleProbe(enabledProbeDelayMs);
        return;
    }

    setAvailable(false);
    if (m_fastAttemptsRemaining > 0) {
        --m_fastAttemptsRemaining;
        scheduleProbe(fastProbeDelayMs);
        return;
    }

    if (m_state != State::Fallback) {
        qInfo() << "Celestina compositor blur unavailable on"
                << m_window->objectName() << "(using opaque fallback)";
    }
    m_state = State::Fallback;
    scheduleProbe(fallbackProbeDelayMs);
}

void PanelBlurController::scheduleProbe(int delayMs)
{
    if (m_window)
        m_probeTimer.start(delayMs);
}

void PanelBlurController::setAvailable(bool available)
{
    if (m_window && m_window->property("compositorBlurAvailable").toBool() != available)
        m_window->setProperty("compositorBlurAvailable", available);
}
