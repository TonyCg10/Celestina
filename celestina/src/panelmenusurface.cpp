#include "panelmenusurface.h"

#include <QDebug>
#include <QMargins>
#include <QScreen>
#include <QSize>

#include "surfacemanager.h"
#include "panelblurcontroller.h"

namespace {
// The card receives the opener's real output-local rectangle, including its
// vertical coordinate. Covering the output is what lets the animation begin at
// that rectangle instead of at a guessed panel edge.
constexpr int ignoreExclusiveZones = -1;

LayerSurfaceSpec menuSpec(
    QScreen *screen,
    PanelMenuSurface::Coverage coverage,
    const QSize &contentSize,
    const QPoint &requestedPosition
)
{
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    anchors |= LayerShellQt::Window::AnchorLeft;

    LayerSurfaceSpec spec;
    spec.scope = coverage == PanelMenuSurface::Coverage::Output
        ? QStringLiteral("celestina-panel-menu")
        : QStringLiteral("celestina-panel-child-menu");
    spec.screen = screen;
    // The available output, with the card placed inside it where the click
    // was. Panel-attached callers inset this carrier at the panel's lower seam;
    // floating and side-attached callers retain the complete output by asking
    // for the default origin.
    //
    // The surface used to be the size of the menu card and positioned by its
    // own margins, which meant a click outside the menu was somebody else's
    // event and the menu stayed up. Covering the output is what lets the menu
    // hear that click — the same correction the focused overlays needed, for
    // the same reason. The card position remains in the content's coordinates;
    // only the carrier's physical origin is expressed through these margins.
    if (coverage == PanelMenuSurface::Coverage::Output) {
        const QSize outputSize = screen ? screen->geometry().size() : QSize();
        const QPoint position(
            qBound(0, requestedPosition.x(),
                   qMax(0, outputSize.width() - 1)),
            qBound(0, requestedPosition.y(),
                   qMax(0, outputSize.height() - 1))
        );
        anchors |= LayerShellQt::Window::AnchorBottom;
        anchors |= LayerShellQt::Window::AnchorRight;
        spec.desiredSize = QSize(0, 0);
        spec.margins = QMargins(position.x(), position.y(), 0, 0);
    } else {
        const QSize outputSize = screen ? screen->geometry().size() : QSize();
        const int maximumX = qMax(0, outputSize.width() - contentSize.width());
        const int maximumY = qMax(0, outputSize.height() - contentSize.height());
        const QPoint position(
            qBound(0, requestedPosition.x(), maximumX),
            qBound(0, requestedPosition.y(), maximumY)
        );
        spec.desiredSize = contentSize;
        spec.margins = QMargins(position.x(), position.y(), 0, 0);
    }
    spec.anchors = anchors;
    spec.exclusiveZone = ignoreExclusiveZones;
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

bool PanelMenuSurface::open(
    QWindow *content,
    QWindow *panel,
    Coverage coverage,
    const QPoint &outputPosition
)
{
    if (!content || !panel || isOpen())
        return false;
    if (coverage == Coverage::Card
        && (content->width() <= 0 || content->height() <= 0)) {
        qWarning() << "Celestina refused a card-sized panel menu without size.";
        return false;
    }

    content->setParent(nullptr);
    m_content = content;
    connect(content, &QWindow::visibleChanged, this, [this](bool visible) {
        contentVisibilityChanged(visible);
    });

    if (!mapLayerSurface(
            content,
            menuSpec(panel->screen(), coverage, content->size(), outputPosition)
        )) {
        qWarning() << "Celestina could not map the panel menu surface.";
        content->disconnect(this);
        m_content.clear();
        return false;
    }

    if (content->metaObject()->indexOfProperty("glassRects") >= 0) {
        auto *blur = new PanelBlurController(content, content);
        blur->start();
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
