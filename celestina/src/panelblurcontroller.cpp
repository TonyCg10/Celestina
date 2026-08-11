#include "panelblurcontroller.h"

#include <QDebug>
#include <QRegion>
#include <QVariant>
#include <QVariantMap>
#include <QWindow>

#include <KWindowEffects>

namespace {
constexpr int fastProbeDelayMs = 16;
constexpr int fastProbeCount = 60;
constexpr int fallbackProbeDelayMs = 2000;
constexpr int enabledProbeDelayMs = 5000;
}

QRegion roundedGlassRegion(const QRect &rect, int requestedRadius)
{
    if (rect.width() <= 0 || rect.height() <= 0)
        return {};

    const int radius = qBound(
        0,
        requestedRadius,
        qMin(rect.width(), rect.height()) / 2
    );
    if (radius == 0)
        return QRegion(rect);

    const int diameter = radius * 2;
    QRegion region(QRect(
        rect.x() + radius,
        rect.y(),
        qMax(0, rect.width() - diameter),
        rect.height()
    ));
    region += QRegion(QRect(
        rect.x(),
        rect.y() + radius,
        rect.width(),
        qMax(0, rect.height() - diameter)
    ));
    region += QRegion(
        QRect(rect.x(), rect.y(), diameter, diameter),
        QRegion::Ellipse
    );
    region += QRegion(
        QRect(rect.right() - diameter + 1, rect.y(), diameter, diameter),
        QRegion::Ellipse
    );
    region += QRegion(
        QRect(rect.x(), rect.bottom() - diameter + 1, diameter, diameter),
        QRegion::Ellipse
    );
    region += QRegion(
        QRect(
            rect.right() - diameter + 1,
            rect.bottom() - diameter + 1,
            diameter,
            diameter
        ),
        QRegion::Ellipse
    );
    return region;
}

bool blurProbeCanUseEffect(
    bool alreadyArmed,
    bool surfaceVisible,
    bool surfaceExposed,
    bool surfaceSized,
    bool effectAvailable,
    bool glassPresent
)
{
    return surfaceVisible
        && surfaceSized
        && effectAvailable
        && glassPresent
        && (surfaceExposed || alreadyArmed);
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
// whole point is to have no edge, and a blurred band ending in the middle of
// the composed scene is as visible as a fill ending there. A pill can, because
// it already *is* an edge: it has a radius and an outline, and the blur stops
// where the pill stops.
//
// So the computed region follows the glass rather than the surface. No
// published glass returns an empty region, but the caller must withdraw the
// effect instead of passing that value to `enableBlurBehind`, which interprets
// an empty region as the complete surface.
QRegion PanelBlurController::glassRegion() const
{
    QRegion region;
    if (!m_window)
        return region;

    // Asked of the surface rather than found by walking its object tree. Two
    // guesses at that tree were wrong, and the failure mode is the worst kind:
    // an empty region is not "blur nothing" to `enableBlurBehind`, it is "blur
    // the whole window".
    QVariantList regions = m_window->property("glassRegions").toList();
    const bool shapesAreTyped = !regions.isEmpty();
    if (!shapesAreTyped)
        regions = m_window->property("glassRects").toList();

    for (const QVariant &value : regions) {
        const QVariantMap shape = shapesAreTyped ? value.toMap() : QVariantMap{};
        const QRectF published = shapesAreTyped
            ? shape.value(QStringLiteral("rect")).toRectF()
            : value.toRectF();
        if (published.width() <= 0 || published.height() <= 0)
            continue;

        const QRect rect = published.toAlignedRect();
        const int radius = shapesAreTyped
            ? qRound(shape.value(QStringLiteral("radius")).toReal())
            : qMin(rect.width(), rect.height()) / 2;
        region += roundedGlassRegion(rect, radius);
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
            m_window.data(), SIGNAL(glassRegionsChanged()),
            this, SLOT(glassRegionsChanged())
        )) {
        qWarning() << "Celestina surface has no glassRegionsChanged signal on"
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

void PanelBlurController::glassRegionsChanged()
{
    m_fastAttemptsRemaining = fastProbeCount;
    m_probeTimer.stop();
    probe();
}

void PanelBlurController::probe()
{
    if (!m_window)
        return;

    const bool surfaceVisible = m_window->isVisible();
    const bool surfaceExposed = m_window->isExposed();
    const bool surfaceSized = !m_window->size().isEmpty();
    const bool effectAvailable = KWindowEffects::isEffectAvailable(
        KWindowEffects::BlurBehind
    );

    // An empty region does not mean "blur nothing": `enableBlurBehind` reads it
    // as the whole window, so a pill lookup that found nothing blurred the
    // complete panel surface and drew a hard full-width edge. The absence of
    // glass has to withdraw the effect rather than ask for it with no shape.
    const QRegion glass = glassRegion();
    if (blurProbeCanUseEffect(
            m_state == State::Enabled,
            surfaceVisible,
            surfaceExposed,
            surfaceSized,
            effectAvailable,
            !glass.isEmpty()
        )) {
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
                    << m_window->property("glassRegions").toList().size()
                    << "shape(s) in" << glass.rectCount() << "region fragment(s)";
        }
        m_state = State::Enabled;
        setAvailable(true);
        scheduleProbe(enabledProbeDelayMs);
        return;
    }

    if (m_state == State::Enabled) {
        if (m_window->isExposed()) {
            KWindowEffects::enableBlurBehind(m_window.data(), false);
            m_window->requestUpdate();
        }
        // Any failed prerequisite revokes the confirmed arm before retries.
        // Only the guarded branch above may use an unexposed surface.
        m_state = State::Pending;
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
