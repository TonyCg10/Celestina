#include "overlaysurface.h"

#include <QDebug>
#include <QScreen>

#include "surfacemanager.h"

namespace {
// How far a corner surface sits from the edges, clear of the panel's own
// exclusive zone.
constexpr int cornerMargin = 16;
// How far the readout sits above the bottom edge: out of the way of anything
// anchored at the bottom, and low enough to read without covering content.
constexpr int readoutMargin = 96;

LayerSurfaceSpec centeredSpec(QScreen *screen, const QSize &size)
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

// A surface that is read rather than used: it never takes the keyboard, never
// activates and never steals focus from what the person is doing. Only where it
// sits differs between the two.
LayerSurfaceSpec quietSpec(
    QScreen *screen,
    const QSize &size,
    const QString &scope,
    LayerShellQt::Window::Anchors anchors,
    const QMargins &margins
)
{
    LayerSurfaceSpec spec;
    spec.scope = scope;
    spec.screen = screen;
    // Anchors without an opposing pair: the surface keeps its own size and is
    // pinned to the edge it names.
    spec.anchors = anchors;
    spec.margins = margins;
    spec.desiredSize = size;
    spec.exclusiveZone = 0;
    spec.layer = LayerShellQt::Window::LayerOverlay;
    // A display that is read, not used: it never takes the keyboard, never
    // activates, and never steals focus from what the person is doing.
    spec.keyboard = LayerShellQt::Window::KeyboardInteractivityNone;
    spec.activateOnShow = false;
    spec.closeOnDismissed = true;
    spec.acceptsFocus = false;
    return spec;
}

LayerSurfaceSpec cornerSpec(QScreen *screen, const QSize &size)
{
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    anchors |= LayerShellQt::Window::AnchorRight;
    return quietSpec(
        screen,
        size,
        QStringLiteral("celestina-toasts"),
        anchors,
        QMargins(0, cornerMargin, cornerMargin, 0)
    );
}

LayerSurfaceSpec readoutSpec(QScreen *screen, const QSize &size)
{
    return quietSpec(
        screen,
        size,
        QStringLiteral("celestina-osd"),
        LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorBottom),
        QMargins(0, 0, 0, readoutMargin)
    );
}
} // namespace

OverlaySurface::OverlaySurface(Placement placement, QObject *parent)
    : QObject(parent)
    , m_placement(placement)
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

    LayerSurfaceSpec spec;
    switch (m_placement) {
    case Placement::Centered:
        spec = centeredSpec(screen, content->size());
        break;
    case Placement::Corner:
        spec = cornerSpec(screen, content->size());
        break;
    case Placement::Readout:
        spec = readoutSpec(screen, content->size());
        break;
    }
    if (!mapLayerSurface(content, spec)) {
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
