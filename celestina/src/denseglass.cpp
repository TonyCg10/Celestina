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

#include "panelblurcontroller.h"
#include "surfacemanager.h"

namespace {

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

void walkSections(QQuickItem *item, QList<DenseGlassShape> *found)
{
    if (!item)
        return;
    const auto children = item->childItems();
    for (QQuickItem *const child : children) {
        if (!child->isVisible())
            continue;
        if (child->objectName() == QLatin1String("celestina-menu-section")
            && child->width() > 0 && child->height() > 0) {
            const QRectF mapped = child->mapRectToScene(
                QRectF(0, 0, child->width(), child->height()));
            // The scene scale that mapped the rectangle also scales the
            // radius; the width ratio is that factor without asking the
            // transform directly.
            const qreal factor =
                child->width() > 0 ? mapped.width() / child->width() : 1.0;
            found->append(DenseGlassShape{
                mapped,
                child->property("cornerRadius").toReal() * factor,
            });
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

    for (const QPointer<QQuickWindow> &companion : std::as_const(m_companions)) {
        if (!companion)
            continue;
        const QColor current = companion->color();
        companion->setColor(current.red() == 0 ? QColor(1, 0, 0, 0)
                                               : QColor(0, 0, 0, 0));
        companion->requestUpdate();
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

    Source &entry = m_sources[source];
    const bool unchanged = entry.window == source
        && entry.screen == source->screen() && entry.shapes.size() == shapes.size()
        && std::equal(
            entry.shapes.cbegin(), entry.shapes.cend(), shapes.cbegin(),
            [](const DenseGlassShape &a, const DenseGlassShape &b) {
                return a.rect == b.rect && qFuzzyCompare(a.radius, b.radius);
            });
    if (unchanged)
        return;

    QScreen *const previous = entry.screen.data();
    entry.window = source;
    entry.screen = source->screen();
    entry.shapes = shapes;
    connect(source, &QObject::destroyed, this,
            [this, source]() { withdraw(source); },
            Qt::UniqueConnection);

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
                    if (rect.width() >= 2 && rect.height() >= 2)
                        scaled.append(DenseGlassShape{rect, shape.radius * keep});
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

QQuickWindow *DenseGlassAggregator::companionFor(QScreen *screen)
{
    QPointer<QQuickWindow> &held = m_companions[screen];
    if (held)
        return held.data();

    // Brought up lazily, on the first dark section, never at shell start:
    // premapping persistent surfaces during startup once stopped the
    // compositor drawing the whole overlay layer, and this surface has
    // nothing to say until a section exists.
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
    // layer above it, so the strong sample always sits beneath their paint,
    // whatever order they map in.
    spec.layer = LayerShellQt::Window::LayerTop;
    spec.keyboard = LayerShellQt::Window::KeyboardInteractivityNone;
    spec.activateOnShow = false;
    spec.closeOnDismissed = false;
    spec.acceptsFocus = false;

    if (!mapLayerSurface(companion, spec)) {
        delete companion;
        return nullptr;
    }
    // It renders nothing and must swallow nothing.
    companion->setMask(QRegion(0, 0, 1, 1));
    held = companion;
    return companion;
}

void DenseGlassAggregator::refresh(QScreen *screen)
{
    if (!screen)
        return;

    QRegion region;
    for (const Source &entry : std::as_const(m_sources)) {
        if (!entry.window || entry.screen.data() != screen)
            continue;
        for (const DenseGlassShape &shape : entry.shapes) {
            region += roundedGlassRegion(
                shape.rect.toAlignedRect(),
                qRound(shape.radius));
        }
    }

    QQuickWindow *const companion = companionFor(screen);
    if (!companion)
        return;

    if (region.isEmpty()) {
        // Withdraw rather than arm: an empty region means "the whole
        // surface" to the effect, which here would be the whole output.
        KWindowEffects::enableBlurBehind(companion, false);
    } else {
        KWindowEffects::enableBlurBehind(companion, true, region);
    }
    kickRender(companion);
    m_quietBeats = 0;
    m_pulse.start();
}
