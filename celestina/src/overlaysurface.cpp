#include "overlaysurface.h"

#include <QDebug>
#include <QScreen>

#include "surfacemanager.h"

namespace {
LayerSurfaceSpec overlaySpec(QScreen *screen, const QSize &size)
{
    LayerSurfaceSpec spec;
    spec.scope = QStringLiteral("celestina-overlay");
    spec.screen = screen;
    // No anchors: an unanchored layer surface with a fixed size is centered on
    // its output by the compositor, which is exactly where a keybind opens a
    // launcher or a clipboard history — there is no click position to anchor
    // to the way the panel's menu has one.
    spec.desiredSize = size;
    spec.exclusiveZone = 0;
    spec.layer = LayerShellQt::Window::LayerOverlay;
    // The one surface kind here that must answer the keyboard on its own,
    // exactly like the panel's menu.
    spec.keyboard = LayerShellQt::Window::KeyboardInteractivityOnDemand;
    spec.activateOnShow = true;
    spec.closeOnDismissed = true;
    spec.acceptsFocus = true;
    return spec;
}
} // namespace

OverlaySurface::OverlaySurface(QObject *parent)
    : QObject(parent)
{
}

OverlaySurface::~OverlaySurface()
{
    close();
}

bool OverlaySurface::open(QWindow *content, QScreen *screen)
{
    if (!content || isOpen())
        return false;

    content->setParent(nullptr);
    m_content = content;
    connect(content, &QWindow::visibleChanged, this, [this](bool visible) {
        contentVisibilityChanged(visible);
    });

    if (!mapLayerSurface(content, overlaySpec(screen, content->size()))) {
        qWarning() << "Celestina could not map an overlay surface.";
        content->disconnect(this);
        m_content.clear();
        return false;
    }

    return true;
}

void OverlaySurface::close()
{
    QWindow *const content = m_content.data();
    m_content.clear();
    if (!content)
        return;

    content->disconnect(this);
    content->hide();
    content->deleteLater();
}

void OverlaySurface::contentVisibilityChanged(bool visible)
{
    if (visible)
        return;

    // The compositor or the overlay itself dismissed the surface; this owns
    // the teardown either way, so no closed-but-tracked window survives.
    close();
    emit dismissed();
}
