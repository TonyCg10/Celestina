#include "panelblurcontroller.h"

#include <QDebug>
#include <QMetaType>
#include <QPointF>
#include <QPolygon>
#include <QRegion>
#include <QVariant>
#include <QVariantMap>
#include <QWindow>

#include <KWindowEffects>

#include <cmath>
#include <limits>

namespace {
constexpr int fastProbeDelayMs = 16;
constexpr int fastProbeCount = 60;
constexpr int fallbackProbeDelayMs = 2000;
constexpr int enabledProbeDelayMs = 5000;
constexpr qsizetype maximumGlassPolygonPoints = 4096;

bool publishedPoint(const QVariant &value, QPoint *point)
{
    QPointF published;
    if (value.metaType().id() == QMetaType::QPointF
        || value.metaType().id() == QMetaType::QPoint) {
        published = value.toPointF();
    } else {
        const QVariantMap coordinates = value.toMap();
        bool xValid = false;
        bool yValid = false;
        const qreal x = coordinates.value(QStringLiteral("x")).toDouble(&xValid);
        const qreal y = coordinates.value(QStringLiteral("y")).toDouble(&yValid);
        if (!xValid || !yValid)
            return false;
        published = QPointF(x, y);
    }

    const qreal roundedX = std::round(published.x());
    const qreal roundedY = std::round(published.y());
    constexpr qreal minimumCoordinate = std::numeric_limits<int>::lowest();
    constexpr qreal maximumCoordinate = std::numeric_limits<int>::max();
    if (!std::isfinite(roundedX) || !std::isfinite(roundedY)
        || roundedX < minimumCoordinate
        || roundedX > maximumCoordinate
        || roundedY < minimumCoordinate
        || roundedY > maximumCoordinate) {
        return false;
    }

    *point = QPoint(static_cast<int>(roundedX), static_cast<int>(roundedY));
    return true;
}

QRegion publishedPolygonRegion(const QVariant &value)
{
    const QVariantList publishedPoints = value.toList();
    if (publishedPoints.size() < 3
        || publishedPoints.size() > maximumGlassPolygonPoints) {
        return {};
    }

    QPolygon polygon;
    polygon.reserve(publishedPoints.size());
    for (const QVariant &published : publishedPoints) {
        QPoint point;
        if (!publishedPoint(published, &point))
            return {};
        polygon.append(point);
    }

    return QRegion(polygon, Qt::WindingFill);
}
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

QRegion glassRegionFromPublishedShapes(
    const QVariantList &typedShapes,
    const QVariantList &legacyRects
)
{
    QRegion region;
    const bool shapesAreTyped = !typedShapes.isEmpty();
    const QVariantList &publishedShapes = shapesAreTyped
        ? typedShapes
        : legacyRects;

    for (const QVariant &value : publishedShapes) {
        const QVariantMap shape = shapesAreTyped ? value.toMap() : QVariantMap{};
        if (shapesAreTyped) {
            const QRegion polygon = publishedPolygonRegion(
                shape.value(QStringLiteral("polygon"))
            );
            if (!polygon.isEmpty()) {
                region += polygon;
                continue;
            }
        }

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

// PANEL-1. The finite glass declared by the surface, and nothing else.
//
// `enableBlurBehind` takes a *region*, and a region is a set of pixels: each one
// is blurred or it is not. There is no gradient blur in the protocol, so every
// blurred area has a hard boundary somewhere. The current panel deliberately
// declares its complete 40-pixel edge-to-edge backdrop as one rectangle; its
// denser pills are paint-only sections above that same compositor sample.
// Menus declare their own bounded body or connector polygon.
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
    return glassRegionFromPublishedShapes(
        m_window->property("glassRegions").toList(),
        m_window->property("glassRects").toList()
    );
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
            // Only the transition is news. A falling membrane re-arms its
            // region on every animation frame now, and logging each one would
            // print dozens of identical lines per opened menu.
            if (m_state != State::Enabled) {
                qInfo() << "Celestina compositor blur armed on"
                        << m_window->objectName() << "at" << m_armedSize
                        << "for"
                        << m_window->property("glassRegions").toList().size()
                        << "shape(s) in" << glass.rectCount()
                        << "region fragment(s)";
            }
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
