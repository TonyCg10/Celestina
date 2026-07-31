#include "panelmenusurface.h"

#include <QDebug>
#include <QRect>
#include <QScreen>

#include "surfacemanager.h"

namespace {
// A menu placed in raw screen coordinates: ignoring exclusive zones is what
// makes its margins readable as "where the click was".
constexpr int ignoreExclusiveZones = -1;

LayerSurfaceSpec menuSpec(
    QScreen *screen,
    const QPoint &globalAnchor,
    const QSize &size
)
{
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    anchors |= LayerShellQt::Window::AnchorLeft;

    QPoint origin = globalAnchor;
    if (screen) {
        const QRect available = screen->geometry();
        origin -= available.topLeft();
        // A menu anchored near an edge stays whole: the compositor is told a
        // position the surface actually fits in, rather than being left to
        // clamp a surface that hangs off the output.
        origin.setX(qBound(0, origin.x(), qMax(0, available.width() - size.width())));
        origin.setY(qBound(0, origin.y(), qMax(0, available.height() - size.height())));
    }

    LayerSurfaceSpec spec;
    spec.scope = QStringLiteral("celestina-panel-menu");
    spec.screen = screen;
    spec.anchors = anchors;
    spec.margins = QMargins(origin.x(), origin.y(), 0, 0);
    // A layer surface anchored to two adjacent edges must state its own size;
    // leaving it to the compositor is what a 0×0 request would do.
    spec.desiredSize = size;
    spec.exclusiveZone = ignoreExclusiveZones;
    spec.layer = LayerShellQt::Window::LayerOverlay;
    // The menu is the one surface here that must answer the keyboard, so it
    // asks for focus on its own rather than inheriting the panel's refusal.
    spec.keyboard = LayerShellQt::Window::KeyboardInteractivityOnDemand;
    spec.activateOnShow = true;
    // Unlike the panel, this surface *should* go away when the compositor
    // dismisses it: a menu that outlives its dismissal is a stuck menu.
    spec.closeOnDismissed = true;
    spec.acceptsFocus = true;
    return spec;
}
} // namespace

PanelMenuSurface::PanelMenuSurface(QObject *parent)
    : QObject(parent)
{
}

PanelMenuSurface::~PanelMenuSurface()
{
    close();
}

bool PanelMenuSurface::open(
    QWindow *content,
    QWindow *panel,
    const QPoint &globalAnchor
)
{
    if (!content || !panel || isOpen())
        return false;

    content->setParent(nullptr);
    m_content = content;
    connect(content, &QWindow::visibleChanged, this, [this](bool visible) {
        contentVisibilityChanged(visible);
    });

    if (!mapLayerSurface(
            content,
            menuSpec(panel->screen(), globalAnchor, content->size())
        )) {
        qWarning() << "Celestina could not map the panel menu surface.";
        content->disconnect(this);
        m_content.clear();
        return false;
    }

    return true;
}

void PanelMenuSurface::close()
{
    QWindow *const content = m_content.data();
    m_content.clear();
    if (!content)
        return;

    content->disconnect(this);
    content->hide();
    content->deleteLater();
}

void PanelMenuSurface::contentVisibilityChanged(bool visible)
{
    if (visible)
        return;

    // The compositor or the menu itself dismissed the surface; this owns the
    // teardown either way, so no closed-but-tracked window survives.
    close();
    emit dismissed();
}
