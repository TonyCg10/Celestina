#include "panelmenusurface.h"

#include <QDebug>
#include <QScreen>
#include <QSize>

#include "surfacemanager.h"

namespace {
// A menu is not itself exclusive, but it must respect every exclusive surface
// the compositor placed before it. Its configured top edge then follows the
// real bottom of a stacked or resized top panel without inventing a global
// position Wayland never exposes.
constexpr int respectExclusiveZones = 0;

LayerSurfaceSpec menuSpec(QScreen *screen)
{
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    anchors |= LayerShellQt::Window::AnchorBottom;
    anchors |= LayerShellQt::Window::AnchorLeft;
    anchors |= LayerShellQt::Window::AnchorRight;

    LayerSurfaceSpec spec;
    spec.scope = QStringLiteral("celestina-panel-menu");
    spec.screen = screen;
    // The whole output, with the card placed inside it where the click was.
    //
    // The surface used to be the size of the menu card and positioned by its
    // own margins, which meant a click outside the menu was somebody else's
    // event and the menu stayed up. Covering the output is what lets the menu
    // hear that click — the same correction the focused overlays needed, for
    // the same reason — and the position moves from the surface's margins into
    // the content's own coordinates.
    spec.anchors = anchors;
    spec.desiredSize = QSize(0, 0);
    spec.exclusiveZone = respectExclusiveZones;
    spec.layer = LayerShellQt::Window::LayerOverlay;
    // The menu must answer the keyboard on its own rather than inheriting the
    // panel's refusal.
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

bool PanelMenuSurface::open(QWindow *content, QWindow *panel)
{
    if (!content || !panel || isOpen())
        return false;

    content->setParent(nullptr);
    m_content = content;
    connect(content, &QWindow::visibleChanged, this, [this](bool visible) {
        contentVisibilityChanged(visible);
    });

    if (!mapLayerSurface(content, menuSpec(panel->screen()))) {
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
