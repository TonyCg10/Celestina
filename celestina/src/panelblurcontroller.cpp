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

// PANEL-1. The pills, and nothing else.
//
// `enableBlurBehind` takes a *region*, and a region is a set of pixels: each one
// is blurred or it is not. There is no gradient blur in the protocol, so every
// blurred area has a hard boundary somewhere. The bar cannot have one — its
// whole point is to have no edge, and a blurred band ending in mid-wallpaper is
// as visible as a fill ending there. A pill can, because it already *is* an
// edge: it has a radius and an outline, and the blur stops where the pill stops.
//
// So the blur follows the glass rather than the surface. An empty region simply
// blurs nothing, which is the honest state of a bar whose readings have all
// withdrawn.
QRegion PanelBlurController::pillRegion() const
{
    QRegion region;
    if (!m_window)
        return region;

    // Asked of the surface rather than found by walking its object tree. Two
    // guesses at that tree were wrong, and the failure mode is the worst kind:
    // an empty region is not "blur nothing" to `enableBlurBehind`, it is "blur
    // the whole window".
    const QVariantList rects = m_window->property("glassRects").toList();
    for (const QVariant &value : rects) {
        const QRectF published = value.toRectF();
        if (published.width() <= 0 || published.height() <= 0)
            continue;

        const QRect rect = published.toAlignedRect();
        const int diameter = qMin(rect.width(), rect.height());
        const int radius = diameter / 2;
        const int capY = rect.y() + (rect.height() - diameter) / 2;

        // Two round caps and their centre match a QML pill. An ellipse over
        // the complete wide rectangle flattens the ends and blurs outside the
        // radius the visible surface actually draws.
        region += QRegion(QRect(rect.x(), capY, diameter, diameter), QRegion::Ellipse);
        region += QRegion(
            QRect(rect.right() - diameter + 1, capY, diameter, diameter),
            QRegion::Ellipse
        );
        region += QRegion(QRect(
            rect.x() + radius,
            rect.y(),
            qMax(0, rect.width() - radius * 2),
            rect.height()
        ));
    }
    return region;
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
    if (!QObject::connect(
            m_window.data(), SIGNAL(glassRectsChanged()),
            this, SLOT(glassRectsChanged())
        )) {
        qWarning() << "Celestina panel has no glassRectsChanged signal on"
                   << m_window->objectName();
    }

    geometryChanged();
}

void PanelBlurController::geometryChanged()
{
    m_armedSize = {};
    m_armedRegion = {};
    m_state = State::Pending;
    m_fastAttemptsRemaining = fastProbeCount;
    m_probeTimer.stop();
    probe();
}

void PanelBlurController::glassRectsChanged()
{
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

    // An empty region does not mean "blur nothing": `enableBlurBehind` reads it
    // as the whole window, so a pill lookup that found nothing blurred all 112
    // pixels of the surface and drew the widest hard edge yet. The absence of
    // glass has to withdraw the effect rather than ask for it with no shape.
    const QRegion glass = pillRegion();
    if (surfaceReady && effectAvailable && !glass.isEmpty()) {
        if (m_state != State::Enabled
            || m_armedSize != m_window->size()
            || m_armedRegion != glass) {
            // ext-background-effect state is double-buffered. Submit a finite,
            // surface-local region and request the frame that commits it.
            KWindowEffects::enableBlurBehind(
                m_window.data(),
                true,
                glass
            );
            m_window->requestUpdate();
            m_armedSize = m_window->size();
            m_armedRegion = glass;
            qInfo() << "Celestina compositor blur armed on"
                    << m_window->objectName() << "at" << m_armedSize
                    << "for"
                    << m_window->property("glassRects").toList().size()
                    << "pill(s) in" << glass.rectCount() << "region fragment(s)";
        }
        m_state = State::Enabled;
        setAvailable(true);
        scheduleProbe(enabledProbeDelayMs);
        return;
    }

    if (m_state == State::Enabled && m_window->isExposed()) {
        KWindowEffects::enableBlurBehind(m_window.data(), false);
        m_window->requestUpdate();
    }
    m_armedSize = {};
    m_armedRegion = {};

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
