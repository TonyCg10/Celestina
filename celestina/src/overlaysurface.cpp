#include "overlaysurface.h"

#include <KWindowEffects>

#include "blurreach.h"

#include <QDebug>
#include <QScreen>

#include "surfacemanager.h"
#include "panelblurcontroller.h"

namespace {
// How far a corner surface sits from the edges, clear of the panel's own
// exclusive zone.
constexpr int cornerMargin = 16;
// A surface placed over everything, including what other surfaces reserved.
constexpr int ignoreExclusiveZones = -1;

int boundedTopInset(QScreen *screen, int requested)
{
    const int nonNegative = qMax(0, requested);
    if (!screen)
        return nonNegative;
    return qMin(nonNegative, qMax(0, screen->geometry().height() - 1));
}

LayerSurfaceSpec centeredSpec(QScreen *screen, int topInset)
{
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    anchors |= LayerShellQt::Window::AnchorBottom;
    anchors |= LayerShellQt::Window::AnchorLeft;
    anchors |= LayerShellQt::Window::AnchorRight;

    LayerSurfaceSpec spec;
    spec.scope = QStringLiteral("celestina-overlay");
    spec.screen = screen;
    // The whole output, with the card centred inside it by the content itself.
    //
    // The surface used to be the size of the card, which left the shell with no
    // way to hear a click outside one: the click went to whatever was behind,
    // and when that was the panel button which opened the overlay, the button
    // asked `toggle()` for a surface that was still mapped. That is the two-
    // click close the author found. Covering the output puts the outside click
    // where the overlay can answer it, in one click, with one focus return —
    // and it is what every keyboard launcher on this protocol already does.
    spec.anchors = anchors;
    // Anchored to all four edges with no size of its own, the compositor sizes
    // the surface to the output — which is what `0 × 0` means here.
    spec.desiredSize = QSize(0, 0);
    // A panel-opened interactive overlay still covers every available pixel
    // for outside dismissal, but its carrier begins at the panel's lower seam.
    // Keybind/floating routes pass zero and retain complete-output coverage.
    spec.margins = QMargins(0, boundedTopInset(screen, topInset), 0, 0);
    // Reserve nothing, and ignore what everything else reserved: the panel's
    // own exclusive zone would otherwise keep this surface off the one strip
    // whose buttons open it.
    spec.exclusiveZone = ignoreExclusiveZones;
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

LayerSurfaceSpec cornerSpec(QScreen *screen, const QSize &size, const QString &scope)
{
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    anchors |= LayerShellQt::Window::AnchorRight;
    return quietSpec(
        screen,
        size,
        scope,
        anchors,
        QMargins(0, cornerMargin, cornerMargin, 0)
    );
}

// The membrane's local mouth is y = 0 and the layer window itself starts at
// the panel's lower seam. Ignore exclusive zones so the explicit inset is
// measured once from the output edge rather than composed with the panel's
// reservation by the compositor.
LayerSurfaceSpec attachedTopRightSpec(
    QScreen *screen,
    const QSize &size,
    const QString &scope,
    int topInset
)
{
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    anchors |= LayerShellQt::Window::AnchorRight;
    LayerSurfaceSpec spec = quietSpec(
        screen,
        size,
        scope,
        anchors,
        QMargins(0, boundedTopInset(screen, topInset), 0, 0)
    );
    spec.exclusiveZone = ignoreExclusiveZones;
    return spec;
}

LayerSurfaceSpec bottomRightSpec(QScreen *screen, const QSize &size, const QString &scope)
{
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorBottom);
    anchors |= LayerShellQt::Window::AnchorRight;
    return quietSpec(
        screen,
        size,
        scope,
        anchors,
        // Flush with the bottom edge on purpose: the display's card enters by
        // physically emerging from it, so the window is the runway and the
        // card keeps its own breathing room inside.
        QMargins(0, 0, cornerMargin, 0)
    );
}

LayerSurfaceSpec bottomCentreSpec(QScreen *screen, const QSize &size, const QString &scope)
{
    return quietSpec(
        screen,
        size,
        scope,
        LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorBottom),
        // Flush like the bottom-right twin, and for the same reason: the
        // block enters by physically emerging from the edge, so the window
        // is the runway and the content keeps its breathing room inside.
        QMargins(0, 0, 0, 0)
    );
}

} // namespace

OverlaySurface::OverlaySurface(
    Placement placement,
    const QString &scope,
    QObject *parent
)
    : QObject(parent)
    , m_placement(placement)
    , m_scope(scope)
{
}

OverlaySurface::~OverlaySurface()
{
    close();
}

bool OverlaySurface::open(QWindow *content, QScreen *screen)
{
    return open(content, screen, m_placement, 0);
}

bool OverlaySurface::open(
    QWindow *content,
    QScreen *screen,
    Placement placement,
    int topInset
)
{
    if (!content)
        return false;

    LayerSurfaceSpec spec;
    switch (placement) {
    case Placement::Centered:
        spec = centeredSpec(screen, topInset);
        break;
    case Placement::Corner:
        spec = cornerSpec(screen, content->size(), m_scope);
        break;
    case Placement::AttachedTopRight:
        spec = attachedTopRightSpec(
            screen, content->size(), m_scope, topInset);
        break;
    case Placement::BottomRight:
        spec = bottomRightSpec(screen, content->size(), m_scope);
        break;
    case Placement::BottomCentre:
        spec = bottomCentreSpec(screen, content->size(), m_scope);
        break;
    }

    // A parked carrier resumes in place instead of remapping — the scene
    // change this state exists to avoid. Only the very window that parked, on
    // the screen it is mapped on, with the anchors of the placement it was
    // mapped with, is a resume; any other request finds this surface occupied.
    if (isParked()) {
        if (content != m_content.data()
            || placement != m_mappedPlacement
            || !screen
            || screen != content->screen()) {
            return false;
        }
        if (!resumeLayerSurface(content, spec))
            return false;
        m_parked = false;
        return true;
    }

    if (isOpen())
        return false;

    content->setParent(nullptr);
    m_content = content;
    connect(content, &QWindow::visibleChanged, this, [this](bool visible) {
        contentVisibilityChanged(visible);
    });

    if (!mapLayerSurface(content, spec)) {
        qWarning() << "Celestina could not map an overlay surface.";
        content->disconnect(this);
        m_content.clear();
        return false;
    }

    // A quiet surface's content grows and shrinks while it is mapped — the
    // toast column gains cards, the display's file gains peeks — and the
    // compositor keeps positioning the surface by the size it was told at
    // map time. A committed buffer larger than that stale size is drawn
    // overflowing the screen edge, which is exactly what a growing
    // bottom-centre stack did. The desired size therefore follows the
    // window; the centered overlays are compositor-sized (0×0) and must not
    // be overridden.
    if (placement != Placement::Centered) {
        auto *layerWindow = LayerShellQt::Window::get(content);
        if (layerWindow) {
            const auto followSize = [layerWindow, content]() {
                // Never zero, and never while leaving. These placements anchor
                // one edge per axis, and layer-shell makes a zero extent on an
                // unopposed axis a *protocol error* — the compositor kills the
                // whole shell for it, not just the surface. A content window
                // whose last card is retiring can pass through an empty height
                // on its way out, so the floor is one pixel and a retiring
                // surface stops following at all: its size from here on is the
                // close animation's business, and nothing it does is worth
                // committing.
                if (content->property("celestinaRetiring").toBool())
                    return;
                const QSize followed = content->size();
                layerWindow->setDesiredSize(
                    QSize(qMax(1, followed.width()), qMax(1, followed.height()))
                );
                content->requestUpdate();
            };
            connect(content, &QWindow::widthChanged, this, followSize);
            connect(content, &QWindow::heightChanged, this, followSize);
        }
    }

    if (content->metaObject()->indexOfProperty("glassRects") >= 0) {
        auto *blur = new PanelBlurController(content, content);
        blur->start();
    }

    m_mappedPlacement = placement;
    return true;
}

bool OverlaySurface::park()
{
    if (!isOpen())
        return false;
    QWindow *const content = m_content.data();
    // A retiring window is mid-departure: its close animation owns it, and
    // the soft close will destroy it. Parking it would leave a carrier this
    // surface believes is resting while its teardown is already scheduled.
    if (content->property("celestinaRetiring").toBool())
        return false;
    if (!parkLayerSurface(content))
        return false;

    m_parked = true;
    return true;
}

void OverlaySurface::close()
{
    QWindow *const content = m_content.data();
    m_content.clear();
    m_parked = false;
    if (!content)
        return;

    content->disconnect(this);
    // The effect object leaves before the surface it hangs on, deliberately.
    //
    // KWindowSystem tears its blur wrapper down from `surfaceDestroyed`, and it
    // does so with `deleteLater` — so the `ext_background_effect_surface_v1`
    // destroy lands a whole event-loop pass *after* Qt has already destroyed the
    // `wl_surface`. A compositor that still holds that surface answers the late
    // request with a fatal protocol error and kills the client; upstream niri
    // #3660 is that exact sequence, reported against Dolphin, which is this same
    // Qt and KWindowSystem stack. `softCloseWindow` already withdraws before its
    // own close; this is the hard path, which did not.
    //
    // Withdrawing here does not merely clear a region: it releases the object
    // while the surface is unquestionably alive, leaving nothing for a late
    // destroy to reference.
    content->setProperty("celestinaRetiring", true);
    withdrawBlur(content);
    content->hide();
    content->deleteLater();
}

void OverlaySurface::contentVisibilityChanged(bool visible)
{
    if (visible)
        return;

    // A parked carrier that hides was not dismissed by anyone this signal is
    // for — the compositor was told it may not dismiss it, and nothing was
    // open for a person to close. Its window is gone either way, so tear down
    // silently and let the next open map fresh.
    const bool parked = m_parked;
    // The compositor or the overlay itself dismissed the surface; this owns
    // the teardown either way, so no closed-but-tracked window survives.
    close();
    if (!parked)
        emit dismissed();
}
