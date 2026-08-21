#include "surfacemanager.h"

#include <QRegion>
#include <QWindow>

LayerShellSupport layerShellSupport(const QString &platformName)
{
    // Qt names its Wayland plugins `wayland`, `wayland-egl` and friends.
    if (platformName.startsWith(QStringLiteral("wayland")))
        return LayerShellSupport::Available;
    if (platformName == QStringLiteral("offscreen"))
        return LayerShellSupport::Headless;

    return LayerShellSupport::Unavailable;
}

bool mapLayerSurface(QWindow *window, const LayerSurfaceSpec &spec)
{
    if (!window)
        return false;

    window->setScreen(spec.screen);
    window->setFlag(Qt::FramelessWindowHint);
    window->setFlag(Qt::WindowDoesNotAcceptFocus, !spec.acceptsFocus);
    if (spec.desiredSize.isValid() && spec.desiredSize.height() > 0)
        window->setHeight(spec.desiredSize.height());

    auto *layerWindow = LayerShellQt::Window::get(window);
    if (!layerWindow)
        return false;

    layerWindow->setScreen(spec.screen);
    layerWindow->setScope(spec.scope);
    layerWindow->setAnchors(spec.anchors);
    layerWindow->setMargins(spec.margins);
    if (spec.desiredSize.isValid())
        layerWindow->setDesiredSize(spec.desiredSize);
    layerWindow->setExclusiveZone(spec.exclusiveZone);
    layerWindow->setLayer(spec.layer);
    layerWindow->setKeyboardInteractivity(spec.keyboard);
    layerWindow->setActivateOnShow(spec.activateOnShow);
    layerWindow->setCloseOnDismissed(spec.closeOnDismissed);

    // Mapping last ensures the output and every layer-shell property are fixed
    // before the compositor creates the surface.
    window->show();
    return true;
}

bool parkLayerSurface(QWindow *window)
{
    if (!window)
        return false;
    auto *layerWindow = LayerShellQt::Window::get(window);
    if (!layerWindow)
        return false;

    // The property is the state every other tenant of this window reads —
    // the blur controller, the QML content — the way `celestinaRetiring`
    // already works.
    window->setProperty("celestinaParked", true);
    window->setMask(QRegion(0, 0, 1, 1));
    window->setFlag(Qt::WindowDoesNotAcceptFocus, true);
    layerWindow->setKeyboardInteractivity(
        LayerShellQt::Window::KeyboardInteractivityNone
    );
    layerWindow->setCloseOnDismissed(false);
    // Double-buffered layer state rides the next commit, which an idle window
    // never schedules on its own.
    window->requestUpdate();
    return true;
}

bool resumeLayerSurface(QWindow *window, const LayerSurfaceSpec &spec)
{
    if (!window)
        return false;
    auto *layerWindow = LayerShellQt::Window::get(window);
    if (!layerWindow)
        return false;

    window->setProperty("celestinaParked", false);
    // An empty mask is Qt's "no mask": the whole surface hears input again.
    window->setMask(QRegion());
    window->setFlag(Qt::WindowDoesNotAcceptFocus, !spec.acceptsFocus);
    layerWindow->setAnchors(spec.anchors);
    layerWindow->setMargins(spec.margins);
    if (spec.desiredSize.isValid())
        layerWindow->setDesiredSize(spec.desiredSize);
    layerWindow->setExclusiveZone(spec.exclusiveZone);
    layerWindow->setKeyboardInteractivity(spec.keyboard);
    layerWindow->setCloseOnDismissed(spec.closeOnDismissed);
    // The surface never unmapped, so `activateOnShow` has no show left to
    // ride; a resumed interactive surface asks for its focus directly.
    if (spec.activateOnShow)
        window->requestActivate();
    window->requestUpdate();
    return true;
}
