#include "denseglass.h"

#include <KWindowEffects>
#include <LayerShellQt/window.h>
#include <QGuiApplication>
#include <QQuickItem>
#include <QQuickWindow>
#include <QRegion>
#include <QScreen>
#include <QTimer>
#include <QVariantAnimation>

#include <cmath>

#include "panelblurcontroller.h"
#include "surfacemanager.h"

namespace {

// How many companion layers stack under the dense sections. Each composes one
// more sample of the session's shared blur over the same rectangles, so this
// is the dense material's strength expressed in surfaces rather than in a
// compositor option no stock niri has. Three, plus the menu's own veil
// region above them, is what measured closest to the reference material.
constexpr int denseCompanionDepth = 3;

// One committed frame is what makes double-buffered effect state land, and an
// idle window schedules none of its own. `requestUpdate` alone is not enough:
// a scene with nothing dirty renders nothing and commits nothing — the on
// screen display measured this and answers it by moving an invisible pixel.
// This window has no scene to move a pixel in, so the dirt is its clear
// colour, toggled between two fully transparent values; the change is
// invisible and the repaint it forces is the commit the effect state rides.
// Without it, the withdrawal sat unarmed for up to a pulse period and the
// strong sample outlived its menu by half a second on the author's recording.
void kickRender(QQuickWindow *window)
{
    const QPointer<QQuickWindow> tracked(window);
    const auto kick = [tracked]() {
        if (!tracked)
            return;
        const QColor current = tracked->color();
        tracked->setColor(current.red() == 0 ? QColor(1, 0, 0, 0)
                                             : QColor(0, 0, 0, 0));
        tracked->requestUpdate();
    };
    kick();
    QTimer::singleShot(50, tracked, kick);
    QTimer::singleShot(250, tracked, kick);
}

// A scene-graph clip is continuous geometry while the compositor effect takes
// an integer QRegion. Round inward: a conservative missing edge pixel is hidden
// beneath the source material, while rounding outward can blur one pixel the
// clipped QML surface never painted — exactly the panel-seam leak this guards.
QRegion containedRegion(const QRectF &rect)
{
    const QRectF normalized = rect.normalized();
    const auto snapIntegral = [](qreal coordinate) {
        const qreal nearest = std::round(coordinate);
        return qAbs(coordinate - nearest) < 0.000001 ? nearest : coordinate;
    };
    const int left = qCeil(snapIntegral(normalized.left()));
    const int top = qCeil(snapIntegral(normalized.top()));
    const int right = qFloor(snapIntegral(normalized.right()));
    const int bottom = qFloor(snapIntegral(normalized.bottom()));
    if (right <= left || bottom <= top)
        return {};
    return QRegion(left, top, right - left, bottom - top);
}

QRegion effectiveClipRegion(QQuickItem *item)
{
    if (!item || !item->window())
        return {};

    // The platform surface is an implicit final clip even though the root
    // content item does not set `clip`. A section outside its carrier cannot
    // authorize a full-output companion to paint there.
    QRegion effective(QRect(QPoint(), item->window()->size()));
    for (QQuickItem *ancestor = item->parentItem(); ancestor;
         ancestor = ancestor->parentItem()) {
        if (!ancestor->clip())
            continue;

        // Celestina's seam and viewport clips use translation plus axis-aligned
        // scale. Mapping the complete bounds through that transform therefore
        // gives the exact scene-space clip rectangle, including nested scales.
        const QRectF mapped = ancestor->mapRectToScene(
            QRectF(0, 0, ancestor->width(), ancestor->height()));
        effective &= containedRegion(mapped);
        if (effective.isEmpty())
            break;
    }
    return effective;
}

QPoint roundedShapeOrigin(const QRectF &rect)
{
    return QPoint(qRound(rect.x()), qRound(rect.y()));
}

void walkSections(QQuickItem *item, QList<DenseGlassShape> *found)
{
    if (!item)
        return;
    const auto children = item->childItems();
    for (QQuickItem *const child : children) {
        if (!child->isVisible())
            continue;
        // A field that has not begun its reveal keeps its sections out of the
        // companions: the dense material cannot fade, so armed early it is a
        // bare milky slab leading the card's paint by several frames — the
        // author's recording showed exactly that on every open. Its sections
        // join the instant the reveal starts, under paint already forming.
        if (child->objectName() == QLatin1String("celestina-soft-menu-field")
            && (!child->property("revealed").toBool()
                || child->opacity() <= 0.0)) {
            continue;
        }
        if (child->objectName() == QLatin1String("celestina-menu-section")
            && child->width() > 0 && child->height() > 0) {
            const QRectF mapped = child->mapRectToScene(
                QRectF(0, 0, child->width(), child->height()));
            // The scene scale that mapped the rectangle also scales the
            // radius; the width ratio is that factor without asking the
            // transform directly.
            const qreal factor =
                child->width() > 0 ? mapped.width() / child->width() : 1.0;
            QRegion relativeClip = effectiveClipRegion(child);
            if (relativeClip.isEmpty()) {
                walkSections(child, found);
                continue;
            }
            relativeClip.translate(-roundedShapeOrigin(mapped));
            const DenseGlassShape shape {
                mapped,
                child->property("cornerRadius").toReal() * factor,
                relativeClip,
            };
            // A fully clipped section paints neither QML nor compositor
            // material. Do not keep an empty shape alive in the aggregator.
            if (!denseGlassRegion(shape).isEmpty())
                found->append(shape);
        }
        walkSections(child, found);
    }
}

} // namespace

QPointF layerSurfaceOriginOnOutput(
    int anchors,
    const QMargins &margins,
    const QSizeF &windowSize,
    const QSizeF &outputSize
)
{
    using LayerWindow = LayerShellQt::Window;
    const bool left = anchors & LayerWindow::AnchorLeft;
    const bool right = anchors & LayerWindow::AnchorRight;
    const bool top = anchors & LayerWindow::AnchorTop;
    const bool bottom = anchors & LayerWindow::AnchorBottom;

    qreal x = 0;
    if (left) {
        x = margins.left();
    } else if (right) {
        x = outputSize.width() - margins.right() - windowSize.width();
    } else {
        x = (outputSize.width() - windowSize.width()) / 2.0;
    }

    qreal y = 0;
    if (top) {
        y = margins.top();
    } else if (bottom) {
        y = outputSize.height() - margins.bottom() - windowSize.height();
    } else {
        y = (outputSize.height() - windowSize.height()) / 2.0;
    }

    return QPointF(x, y);
}

QList<DenseGlassShape> collectDenseSections(QQuickWindow *window)
{
    QList<DenseGlassShape> found;
    if (window)
        walkSections(window->contentItem(), &found);
    return found;
}

QRegion denseGlassRegion(const DenseGlassShape &shape)
{
    QRegion region = roundedGlassRegion(
        shape.rect.toAlignedRect(), qRound(shape.radius));
    if (!shape.clipRegion.isEmpty()) {
        QRegion clip = shape.clipRegion;
        clip.translate(roundedShapeOrigin(shape.rect));
        region &= clip;
    }
    return region;
}

DenseGlassAggregator &DenseGlassAggregator::instance()
{
    static DenseGlassAggregator aggregator;
    return aggregator;
}

DenseGlassAggregator::DenseGlassAggregator(QObject *parent)
    : QObject(parent)
{
    m_pulse.setInterval(500);
    connect(&m_pulse, &QTimer::timeout, this, &DenseGlassAggregator::pulse);
}

void DenseGlassAggregator::pulse()
{
    bool anythingArmed = false;
    for (const Source &entry : std::as_const(m_sources))
        anythingArmed = anythingArmed || !entry.shapes.isEmpty();

    for (const QList<QPointer<QQuickWindow>> &screenCompanions
             : std::as_const(m_companions)) {
        for (const QPointer<QQuickWindow> &companion : screenCompanions) {
            if (!companion)
                continue;
            const QColor current = companion->color();
            companion->setColor(current.red() == 0 ? QColor(1, 0, 0, 0)
                                                   : QColor(0, 0, 0, 0));
            companion->requestUpdate();
        }
    }

    if (anythingArmed) {
        m_quietBeats = 0;
        return;
    }
    // A few beats past empty, so the withdrawal itself is committed; then
    // silence, because a resting companion has nothing to say.
    if (++m_quietBeats > 4)
        m_pulse.stop();
}

void DenseGlassAggregator::publish(
    QWindow *source,
    const QList<DenseGlassShape> &shapes
)
{
    if (!source || !source->screen())
        return;

    // Whether this source is new decides whether it still needs a destroyed
    // hook. `Qt::UniqueConnection` cannot answer that: it is documented not to
    // work with a functor, and asserts in a debug build rather than
    // deduplicating — which is what aborted the surface-manager regression.
    const bool isNew = !m_sources.contains(source);
    Source &entry = m_sources[source];
    const bool unchanged = entry.window == source
        && entry.screen == source->screen() && entry.shapes.size() == shapes.size()
        && std::equal(
            entry.shapes.cbegin(), entry.shapes.cend(), shapes.cbegin(),
            [](const DenseGlassShape &a, const DenseGlassShape &b) {
                return a.rect == b.rect && qFuzzyCompare(a.radius, b.radius)
                    && a.clipRegion == b.clipRegion;
            });
    if (unchanged)
        return;

    QScreen *const previous = entry.screen.data();
    entry.window = source;
    entry.screen = source->screen();
    entry.shapes = shapes;
    if (isNew) {
        connect(source, &QObject::destroyed, this,
                [this, source]() { withdraw(source); });
    }

    refresh(source->screen());
    if (previous && previous != source->screen())
        refresh(previous);
}

void DenseGlassAggregator::retire(QWindow *source)
{
    const auto it = m_sources.constFind(source);
    if (it == m_sources.cend() || it->shapes.isEmpty()) {
        withdraw(source);
        return;
    }

    const QList<DenseGlassShape> resting = it->shapes;
    const QPointer<QWindow> tracked(source);
    auto *collapse = new QVariantAnimation(this);
    collapse->setStartValue(1.0);
    collapse->setEndValue(0.0);
    collapse->setDuration(80);
    collapse->setEasingCurve(QEasingCurve::OutCubic);
    connect(collapse, &QVariantAnimation::valueChanged, this,
            [this, tracked, resting](const QVariant &value) {
                if (!tracked)
                    return;
                const qreal keep = value.toReal();
                QList<DenseGlassShape> scaled;
                scaled.reserve(resting.size());
                for (const DenseGlassShape &shape : resting) {
                    const QPointF centre = shape.rect.center();
                    QRectF rect(0, 0, shape.rect.width() * keep,
                                shape.rect.height() * keep);
                    rect.moveCenter(centre);
                    if (rect.width() >= 2 && rect.height() >= 2) {
                        // The clip is stored relative to the shape so ordinary
                        // surface-to-output translation stays a one-property
                        // operation. Retirement is different: the rectangle
                        // shrinks within a fixed output clip, so compensate for
                        // its moving top-left before publishing the next frame.
                        QRegion fixedClip = shape.clipRegion;
                        fixedClip.translate(
                            roundedShapeOrigin(shape.rect)
                            - roundedShapeOrigin(rect));
                        scaled.append(DenseGlassShape{
                            rect,
                            shape.radius * keep,
                            fixedClip,
                        });
                    }
                }
                publish(tracked.data(), scaled);
            });
    connect(collapse, &QVariantAnimation::finished, this,
            [this, tracked]() {
                if (tracked)
                    withdraw(tracked.data());
            });
    collapse->start(QAbstractAnimation::DeleteWhenStopped);
}

void DenseGlassAggregator::withdraw(QWindow *source)
{
    const auto it = m_sources.constFind(source);
    if (it == m_sources.cend())
        return;
    QScreen *const screen = it->screen.data();
    m_sources.erase(it);
    if (screen)
        refresh(screen);
}

QList<QPointer<QQuickWindow>> DenseGlassAggregator::companionsFor(QScreen *screen)
{
    QList<QPointer<QQuickWindow>> &held = m_companions[screen];
    held.removeIf([](const QPointer<QQuickWindow> &w) { return w.isNull(); });
    if (held.size() >= denseCompanionDepth)
        return held;

    // Brought up lazily, on the first dark section, never at shell start:
    // premapping persistent surfaces during startup once stopped the
    // compositor drawing the whole overlay layer, and these have nothing to
    // say until a section exists. Mapped in order, so each sits above the
    // one before it and samples its result.
    while (held.size() < denseCompanionDepth) {
        auto *companion = new QQuickWindow();
        companion->setColor(Qt::transparent);
        companion->setTitle(QStringLiteral("Celestina dense glass"));

        LayerSurfaceSpec spec;
        spec.scope = QStringLiteral("celestina-dense-glass");
        spec.screen = screen;
        auto anchors =
            LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
        anchors |= LayerShellQt::Window::AnchorBottom;
        anchors |= LayerShellQt::Window::AnchorLeft;
        anchors |= LayerShellQt::Window::AnchorRight;
        spec.anchors = anchors;
        spec.desiredSize = QSize(0, 0);
        spec.exclusiveZone = -1;
        // The top layer, deliberately: every publisher lives on the overlay
        // layer above it, so the strong sample always sits beneath their
        // paint, whatever order they map in.
        spec.layer = LayerShellQt::Window::LayerTop;
        spec.keyboard = LayerShellQt::Window::KeyboardInteractivityNone;
        spec.activateOnShow = false;
        spec.closeOnDismissed = false;
        spec.acceptsFocus = false;

        if (!mapLayerSurface(companion, spec)) {
            delete companion;
            break;
        }
        // They render nothing and must swallow nothing.
        companion->setMask(QRegion(0, 0, 1, 1));
        held.append(companion);
    }
    return held;
}

void DenseGlassAggregator::refresh(QScreen *screen)
{
    if (!screen)
        return;

    QRegion region;
    for (const Source &entry : std::as_const(m_sources)) {
        if (!entry.window || entry.screen.data() != screen)
            continue;
        for (const DenseGlassShape &shape : entry.shapes)
            region += denseGlassRegion(shape);
    }

    const QList<QPointer<QQuickWindow>> companions = companionsFor(screen);
    for (const QPointer<QQuickWindow> &companion : companions) {
        if (!companion)
            continue;
        if (region.isEmpty()) {
            // A companion with no rectangles is unmapped, not merely
            // disarmed. Withdrawing the region alone leaves a mapped surface
            // with *no* region, and the compositor's per-namespace rule then
            // applies its saturation and noise to the surface's whole
            // geometry — the whole output. Three resting companions
            // multiplied the session's saturation by ~1.95 permanently, which
            // the author lived with as "normal" until a menu opened, showed
            // the true wallpaper, and read as the menu desaturating the
            // screen.
            KWindowEffects::enableBlurBehind(companion.data(), false);
            companion->setVisible(false);
        } else {
            if (!companion->isVisible())
                companion->setVisible(true);
            KWindowEffects::enableBlurBehind(companion.data(), true, region);
            kickRender(companion.data());
        }
    }
    m_quietBeats = 0;
    m_pulse.start();
}
