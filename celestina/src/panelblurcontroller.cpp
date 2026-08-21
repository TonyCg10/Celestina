#include "panelblurcontroller.h"

#include "diagnosticjournal.h"

#include <QEvent>

#include <QDebug>
#include <QMetaType>
#include <QPointF>
#include <QPolygon>
#include <QRegion>
#include <QVariant>
#include <QVariantMap>
#include <QWindow>

#include <KWindowEffects>
#include <LayerShellQt/window.h>
#include <QQuickWindow>
#include <QScreen>

#include "blurreach.h"
#include "denseglass.h"

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
    bool glassPresent,
    bool surfaceRetiring
)
{
    return !surfaceRetiring
        && surfaceVisible
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
    const QRegion published = glassRegionFromPublishedShapes(
        m_window->property("glassRegions").toList(),
        m_window->property("glassRects").toList()
    );

    // Eroded by one pixel before it is armed. A region is a set of whole
    // pixels and the compositor cuts the effect at its boundary with no
    // antialiasing, so a curved edge is always a one-pixel staircase — which
    // the strong colour-summary blur (2026-08-14) turned from invisible into
    // the author's "sawtooth". Pulling the effect one pixel inside the
    // painted silhouette tucks that staircase under the material's own
    // antialiased edge and the seam stroke it wears now.
    //
    // The erosion is the intersection of the four one-pixel translates — a
    // true 4-neighbourhood erosion, cheap in region arithmetic — with one
    // amendment: a neighbour that falls off the window counts as present, so
    // an edge that runs along the window's own boundary is not eaten. The
    // panel's backdrop meets the screen's top edge exactly; the first cut of
    // this erosion shaved that row and opened a one-pixel bright seam over
    // the whole bar. A region so thin the erosion would consume it keeps its
    // published shape: a hairline connector with a step beats no connector.
    const QRect frame(0, 0,
                      qMax(1, m_window->width()), qMax(1, m_window->height()));
    QRegion eroded = published;
    const struct { QPoint step; QRect keep; } sides[] = {
        {QPoint(1, 0), QRect(frame.x(), frame.y(), 1, frame.height())},
        {QPoint(-1, 0),
         QRect(frame.right(), frame.y(), 1, frame.height())},
        {QPoint(0, 1), QRect(frame.x(), frame.y(), frame.width(), 1)},
        {QPoint(0, -1),
         QRect(frame.x(), frame.bottom(), frame.width(), 1)},
    };
    for (const auto &side : sides)
        eroded &= published.translated(side.step) + side.keep;
    eroded &= published;
    return eroded.isEmpty() ? published : eroded;
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
            if (visible) {
                geometryChanged();
            } else {
                // A hidden window's sections are gone now, not when its
                // deferred destruction runs: the companion's strong sample
                // outliving the menu was a ghost of blurred rectangles.
                DenseGlassAggregator::instance().withdraw(m_window.data());
            }
        }
    );
    // Exposure, not visibility: the effect region is double-buffered surface
    // state, and one committed before the compositor acknowledged the surface
    // is silently dropped. A card-sized child menu armed its blur in that
    // window often enough that its glass came and went between openings —
    // re-arming on the real expose event is what makes the commit land on a
    // surface that exists.
    m_window->installEventFilter(this);
    if (!QObject::connect(
            m_window.data(), SIGNAL(glassRegionsChanged()),
            this, SLOT(glassRegionsChanged())
        )) {
        qWarning() << "Celestina surface has no glassRegionsChanged signal on"
                   << m_window->objectName();
    }

    geometryChanged();
}

bool PanelBlurController::eventFilter(QObject *watched, QEvent *event)
{
    // Only while the region has not landed: the race this exists for is a
    // commit made before the compositor acknowledged the surface, which can
    // only happen around the first exposure. Re-arming on every expose kept
    // resetting an already-enabled region — once per provider tick on the
    // panel — which is churn with nothing to buy.
    if (watched == m_window.data() && event->type() == QEvent::Expose
        && m_window && m_window->isExposed() && m_state != State::Enabled) {
        geometryChanged();
    }
    return QObject::eventFilter(watched, event);
}

void PanelBlurController::geometryChanged()
{
    m_armedSize = {};
    m_armedRegion = {};
    m_state = State::Pending;
    m_fastAttemptsRemaining = fastProbeCount;
    m_probeTimer.stop();
    probe();
    publishDenseSections();
}

void PanelBlurController::glassRegionsChanged()
{
    m_fastAttemptsRemaining = fastProbeCount;
    m_probeTimer.stop();
    probe();
    publishDenseSections();
}

// The dark sections' second, stronger blur. The veil's own region above rides
// this window; the sections' rides the per-output companion surface, because
// the compositor grants one blur strength per surface and the author's
// material (2026-08-14) wants the colour summary only under the dark cards.
// Published on the same beats as the veil's region, so the two stay one
// movement through every fall and reflow.
void PanelBlurController::publishDenseSections()
{
    auto *quick = qobject_cast<QQuickWindow *>(m_window.data());
    if (!quick)
        return;
    // DenseGlassAggregator owns the collapse once softCloseWindow marks this
    // carrier. A late geometry or region callback must not republish the
    // resting shapes over that retirement.
    if (quick->property("celestinaRetiring").toBool())
        return;
    auto *layer = LayerShellQt::Window::get(quick);
    if (!layer)
        return;
    // The panel shares the top layer with the companion and cannot be
    // guaranteed above it; its sections keep the veil strength.
    if (layer->scope() == QLatin1String("celestina-panel"))
        return;
    QScreen *const screen = quick->screen();
    if (!screen)
        return;

    const QPointF origin = layerSurfaceOriginOnOutput(
        int(layer->anchors()),
        layer->margins(),
        QSizeF(quick->width(), quick->height()),
        QSizeF(screen->geometry().size())
    );
    QList<DenseGlassShape> shapes = collectDenseSections(quick);
    for (DenseGlassShape &shape : shapes)
        shape.rect.translate(origin);
    DenseGlassAggregator::instance().publish(quick, shapes);
}

void PanelBlurController::probe()
{
    if (!m_window)
        return;

    // A live `QWindow` is not a live Wayland surface. Qt destroys the platform
    // window — and with it the `wl_surface` — the moment a window hides, while
    // the C++ object survives for as long as anything holds it. `QPointer`
    // cannot see that difference, so every path below has to.
    //
    // This is not defensive tidiness: it is the crash that ended the first two
    // live migrations. `KWindowEffects::enableBlurBehind` becomes
    // `ext_background_effect_surface_v1.set_blur_region`, and the compositor
    // answers a request whose `wl_surface` is gone with a *fatal* protocol
    // error — the whole shell dies, every output at once. A menu that hid
    // while this controller still had a probe queued was enough.
    //
    // Both directions are refused, arm and withdraw alike. There is nothing to
    // withdraw from a surface the compositor has already forgotten.
    if (!m_window->handle()) {
        // The effect died with the surface. Forget the armed state so a window
        // that maps again re-arms from scratch rather than believing a region
        // it no longer has.
        m_state = State::Pending;
        m_armedSize = {};
        m_armedRegion = {};
        m_probeTimer.stop();
        return;
    }

    // `softCloseWindow` owns the deliberate 60 ms weak-blur withdrawal under
    // fading paint. Stop probing without changing that effect here: otherwise
    // a pending capability timer can re-arm the region after the soft close
    // has begun (or withdraw it too early and expose the material swap).
    const bool surfaceRetiring =
        m_window->property("celestinaRetiring").toBool();
    if (surfaceRetiring) {
        m_probeTimer.stop();
        return;
    }

    // A parked carrier is deliberately dark (SURF-1), and this must precede
    // the arming branch: parked with its glass still published — the hard
    // close's path — the probe re-armed the blur and the floating ghost card
    // glowed. Withdraw whatever is still armed, then stop quietly, without
    // the fallback record; the resume's glass publication restarts this.
    if (m_window->property("celestinaParked").toBool()) {
        if (m_state == State::Enabled) {
            withdrawBlur(m_window.data());
            m_window->requestUpdate();
        }
        m_state = State::Pending;
        m_armedSize = {};
        m_armedRegion = {};
        m_probeTimer.stop();
        setAvailable(false);
        return;
    }

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
            !glass.isEmpty(),
            surfaceRetiring
        )) {
        if (m_state != State::Enabled
            || m_armedSize != m_window->size()
            || m_armedRegion != glass) {
            // ext-background-effect state is double-buffered. Submit a finite,
            // surface-local region and request the frame that commits it.
            armBlur(m_window.data(), glass);
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
                // Also to the journal: a nested session's console is not
                // reachable from outside it, and "which surface had its blur
                // armed, how large" is exactly the bounded technical fact a
                // glass-less menu needs answered.
                const QRect bounds = glass.boundingRect();
                DiagnosticJournal::instance().record(
                    DiagnosticJournal::Record(
                        DiagnosticJournal::Level::Info,
                        QStringLiteral("blur.armed"))
                        .text(QStringLiteral("surface"), m_window->objectName())
                        .number(QStringLiteral("shapes"),
                                m_window->property("glassRegions")
                                    .toList().size())
                        .number(QStringLiteral("region_x"), bounds.x())
                        .number(QStringLiteral("region_y"), bounds.y())
                        .number(QStringLiteral("region_width"), bounds.width())
                        .number(QStringLiteral("region_height"),
                                bounds.height())
                        .number(QStringLiteral("window_width"),
                                m_armedSize.width())
                        .number(QStringLiteral("window_height"),
                                m_armedSize.height())
                );
            }
        }
        m_state = State::Enabled;
        setAvailable(true);
        scheduleProbe(enabledProbeDelayMs);
        return;
    }

    if (m_state == State::Enabled) {
        // Regardless of *exposure*: an idle Wayland window's exposed flag flaps
        // (measured on the nested session — a mapped, committing surface
        // reported unexposed), and a withdraw skipped on that flag left the
        // armed region blurring an empty rectangle over the wallpaper for as
        // long as the persistent surface lived. The effect state is
        // double-buffered and simply rides the window's next commit — which the
        // display's heartbeat guarantees.
        //
        // Visibility is a different matter and `withdrawBlur` enforces it: a
        // hidden window has no `wl_surface` left to withdraw from, and this
        // exact call on a just-closed menu is what killed the shell live. The
        // local state below is reset either way, so a surface that comes back
        // re-arms from scratch.
        withdrawBlur(m_window.data());
        m_window->requestUpdate();
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
        // Which precondition failed, per surface, and therefore per output.
        //
        // "No blur on that monitor" was unanswerable before this: the positive
        // `blur.armed` record says nothing about the surfaces that never armed,
        // and on a three-output session the interesting case is the output that
        // differs from its neighbours. Each of these is one of the reasons
        // `blurProbeCanUseEffect` can refuse, recorded separately so the answer
        // is read rather than guessed at.
        DiagnosticJournal::instance().record(
            DiagnosticJournal::Record(
                DiagnosticJournal::Level::Info,
                QStringLiteral("blur.unavailable"))
                .text(QStringLiteral("surface"), m_window->objectName())
                .text(QStringLiteral("output"),
                      m_window->screen() ? m_window->screen()->name()
                                         : QString())
                .flag(QStringLiteral("visible"), surfaceVisible)
                .flag(QStringLiteral("exposed"), surfaceExposed)
                .flag(QStringLiteral("sized"), surfaceSized)
                .flag(QStringLiteral("effect_available"), effectAvailable)
                .flag(QStringLiteral("has_glass"), !glass.isEmpty())
                .number(QStringLiteral("window_width"), m_window->width())
                .number(QStringLiteral("window_height"), m_window->height())
        );
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
