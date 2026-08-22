#include "denseglass.h"

#include <KWindowEffects>

#include "blurreach.h"
#include "commitnudge.h"
#include "diagnosticjournal.h"
#include <LayerShellQt/window.h>
#include <QDateTime>
#include <QGuiApplication>
#include <QQuickItem>
#include <QQuickWindow>
#include <QSGSimpleRectNode>
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

// The parked state's effect region. One pixel, not empty and not withdrawn,
// and both alternatives are measured defects: `withdrawBlur` removes the
// client's region entirely, and a mapped surface with a matching layer-rule
// and no region carries the rule's effect over its whole geometry — the
// resting-companion saturation of 2026-08-15. An *empty* region is no better:
// KWindowSystem reads it as "everything". With one pixel the compositor clips
// the effect to that pixel, which keeps the parked surface invisible and the
// output's effect pipeline warm, so re-arming is a region update rather than
// an on/off transition.
const QRegion parkedCompanionRegion(0, 0, 1, 1);

// One committed frame is what makes double-buffered effect state land, and an
// idle window schedules none of its own. `requestUpdate` alone is not enough:
// a scene with nothing dirty renders nothing and commits nothing — the on
// screen display measured this and answers it by moving an invisible pixel.
// This window has no scene to move a pixel in, so the dirt is its clear
// colour, toggled between two fully transparent values; the change is
// invisible and the repaint it forces is the commit the effect state rides.
// Without it, the withdrawal sat unarmed for up to a pulse period and the
// strong sample outlived its menu by half a second on the author's recording.
// The dirt used to be the clear colour, and the colour leaked: see
// `commitnudge.h`, which now owns the one honest way to force a commit.
void kickRender(QQuickWindow *window)
{
    const QPointer<QQuickWindow> tracked(window);
    const auto kick = [tracked]() { nudgeSurfaceCommit(tracked.data()); };
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
                || child->opacity() <= 0.0
                // A retiring or fully faded field keeps its sections out the
                // same way an unrevealed one does. The same-icon close parks
                // the carrier without the soft retirement, so the field stays
                // revealed while its paint is already gone — and a late
                // publication collected through that state re-armed the
                // companion with the settled card's sections: the author's
                // standing blue slab (2026-08-21 14:22).
                || child->property("retiring").toBool()
                || [child]() {
                       // Valid-and-zero, not merely zero: a harness field
                       // without the property must keep the collector's old
                       // answer.
                       const QVariant presented =
                           child->property("presentationOpacity");
                       return presented.isValid() && presented.toReal() <= 0.0;
                   }())) {
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

    // An output that leaves takes its companions with it.
    //
    // They are keyed by raw `QScreen *`, and a screen Qt has destroyed leaves
    // that key dangling: the entry survives, its windows are reassigned by Qt
    // to whatever screen remains, and they go on applying the dense namespace's
    // compositor rule there — a ghost from an unplugged monitor saturating a
    // different one. Unplugging is not rare on this session; it is how the
    // author works.
    if (auto *application = qApp) {
        connect(application, &QGuiApplication::screenRemoved, this,
                [this](QScreen *screen) { forgetScreen(screen); });
    }
}

void DenseGlassAggregator::setFullscreenOutputs(const QStringList &outputs)
{
    const QSet<QString> next(outputs.begin(), outputs.end());
    if (next == m_fullscreenOutputs)
        return;
    m_fullscreenOutputs = next;

    // Only outputs whose companions are resting change anything now: a parked
    // companion on a newly fullscreen output unmaps, and a parked companion
    // whose output was just given back re-parks lazily on its next refresh
    // rather than being remapped for nobody. Live sections are left alone.
    for (auto it = m_companions.constBegin(); it != m_companions.constEnd();
         ++it) {
        QScreen *const screen = it.key();
        if (!screen || !m_fullscreenOutputs.contains(screen->name()))
            continue;
        refresh(screen);
    }
}

void DenseGlassAggregator::forgetScreen(QScreen *screen)
{
    const auto held = m_companions.take(screen);
    for (const QPointer<QQuickWindow> &companion : held) {
        if (!companion)
            continue;
        // Withdrawn before it dies, like every other effect-bearing surface in
        // this shell: a destroy that reaches the compositor after the surface
        // is gone is a fatal protocol error for the whole client.
        withdrawBlur(companion.data());
        companion->setVisible(false);
        companion->deleteLater();
    }
}

void DenseGlassAggregator::pulse()
{
    bool anythingArmed = false;
    for (const Source &entry : std::as_const(m_sources))
        anythingArmed = anythingArmed || !entry.shapes.isEmpty();

    // The same honest dirt as everywhere else. This loop kept its own copy
    // of the old clear-colour toggle after the kick's was fixed, so the red
    // unit went on blinking at the pulse's beat for as long as any glass was
    // armed — which is exactly "while a menu is open", where the author kept
    // seeing it.
    for (const QList<QPointer<QQuickWindow>> &screenCompanions
             : std::as_const(m_companions)) {
        for (const QPointer<QQuickWindow> &companion : screenCompanions) {
            if (!companion)
                continue;
            nudgeSurfaceCommit(companion.data());
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
            [this, tracked, resting, collapse](const QVariant &value) {
                if (!tracked)
                    return;
                // The collapse belongs to the retirement. A park or a revive
                // ends that retirement early, and a collapse that kept going
                // overwrote the reopened menu's live shapes with these
                // shrinking ones — then withdrew them outright at `finished`.
                if (!tracked->property("celestinaRetiring").toBool()) {
                    collapse->stop();
                    return;
                }
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
                // Only a retirement that is still in force ends in withdrawal;
                // an interrupted one already had its shapes withdrawn (a park)
                // or republished live (a revive) by whoever interrupted it.
                if (tracked && tracked->property("celestinaRetiring").toBool())
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

QList<QPointer<QQuickWindow>> DenseGlassAggregator::companionsFor(
    QScreen *screen,
    const QRegion &region
)
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

        // Armed before the surface exists, and that ordering is the whole
        // repair. `mapLayerSurface` ends in `show()`, so a companion created
        // the old way was mapped with no region for at least the frame that
        // `show()` commits — and the compositor reads a mapped surface with no
        // region as "this effect covers the whole geometry", which is the
        // entire output. The author saw it as a colour flash on every menu
        // opening, and it survived the earlier repair because that one only
        // withdrew *after* the surface was already up.
        //
        // `armBlur` deliberately does not require visibility: KWindowSystem
        // caches the region and applies it on the first expose, so the
        // compositor's very first sight of this surface already carries it.
        armBlur(companion, region);

        if (!mapLayerSurface(companion, spec)) {
            // Per screen, and silently until now. A companion that will not map
            // leaves *this* output with fewer blur samples than its neighbours
            // — the dark cards summarize less, or stop summarizing at all — and
            // that is exactly the shape of "one monitor has no glass" with
            // nothing anywhere to say so. The screen is named because on a
            // multi-output session the interesting fact is which one differs.
            qCritical() << "Celestina could not map a dense-glass companion on"
                        << (screen ? screen->name() : QStringLiteral("(no screen)"))
                        << "— that output keeps" << held.size()
                        << "of" << denseCompanionDepth << "samples";
            DiagnosticJournal::instance().record(
                DiagnosticJournal::Record(
                    DiagnosticJournal::Level::Critical,
                    QStringLiteral("glass.companion.unmapped"))
                    .text(QStringLiteral("output"),
                          screen ? screen->name() : QString())
                    .number(QStringLiteral("mapped"), held.size())
                    .number(QStringLiteral("expected"), denseCompanionDepth)
            );
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

    if (region.isEmpty()) {
        // Nothing to summarize. Existing companions are parked below — mapped,
        // invisible, their effect clipped to one pixel — instead of unmapped,
        // because unmapping per popup is a scene change on this output and a
        // scene change per popup is the measured physical flicker of exactly
        // this monitor (2026-08-18). The park is indefinite: the one tenant
        // it yields to is a fullscreen window, and the compositor names those
        // outputs itself now. New companions are still not created here,
        // because a companion brought up without a region is the whole-output
        // flash this class exists to avoid.
        const bool yields = m_fullscreenOutputs.contains(screen->name());
        const auto resting = m_companions.value(screen);
        for (const QPointer<QQuickWindow> &companion : resting) {
            if (!companion || !companion->isVisible())
                continue;
            if (yields) {
                // Withdrawn while still visible, then hidden: the order every
                // effect-bearing surface in this shell must keep, because a
                // withdraw sent after the surface hid is a fatal protocol
                // error for the whole client.
                withdrawBlur(companion.data());
                companion->setVisible(false);
            } else {
                armBlur(companion.data(), parkedCompanionRegion);
                kickRender(companion.data());
            }
        }
        m_quietBeats = 0;
        m_pulse.start();
        return;
    }

    const QList<QPointer<QQuickWindow>> companions =
        companionsFor(screen, region);
    for (const QPointer<QQuickWindow> &companion : companions) {
        if (!companion)
            continue;
        // Armed first, shown second, on every refresh as on creation. A
        // companion that is already up simply takes the new rectangles.
        armBlur(companion.data(), region);
        if (!companion->isVisible())
            companion->setVisible(true);
        kickRender(companion.data());
    }
    m_quietBeats = 0;
    m_pulse.start();
}
